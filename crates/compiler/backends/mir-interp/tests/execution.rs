use pop_backend_mir_interp::{
    ExecutionError, ForeignAdapterRegistrationError, MirInterpreter, MirValue,
    ReferenceRuntimeEvent, TypedForeignAdapter,
};
use pop_driver::{FrontEndBubbleInput, FrontEndModule, analyze_bubble};
use pop_foundation::{
    BubbleId, BuiltinTypeId, EnumCaseId, FieldId, FileId, ModuleId, NamespaceId, ResultCaseId,
    SymbolId, UnionCaseId,
};
use pop_mir::{lower_hir_bubble, optimize_mir, parse_mir_dump};
use pop_runtime_collector::GenerationalRuntime;
use pop_runtime_interface::{
    ForeignAddress, PanicKind, RuntimeAdapter, RuntimeFailure, Trap, TrapKind, UnwindReason,
};
use pop_source::SourceFile;
use pop_types::{FloatKind, FloatValue, IntegerKind, IntegerValue};

fn executable_source(text: &str) -> (pop_mir::MirBubble, pop_types::TypeArena) {
    let source = SourceFile::new(FileId::from_raw(0), "src/main.pop", text).expect("source");
    let front_end = analyze_bubble(FrontEndBubbleInput::new(
        BubbleId::from_raw(0),
        NamespaceId::from_raw(0),
        Vec::new(),
        vec![FrontEndModule::new(ModuleId::from_raw(0), source)],
    ));
    assert!(
        front_end.diagnostics().is_empty(),
        "{}",
        front_end.diagnostic_snapshot()
    );
    let mir = lower_hir_bubble(front_end.hir().expect("HIR"), front_end.types()).expect("MIR");
    (mir, front_end.types().clone())
}

fn executable_source_function(
    text: &str,
    function_name: &str,
) -> (pop_mir::MirBubble, pop_types::TypeArena, SymbolId) {
    let source = SourceFile::new(FileId::from_raw(0), "src/main.pop", text).expect("source");
    let front_end = analyze_bubble(FrontEndBubbleInput::new(
        BubbleId::from_raw(0),
        NamespaceId::from_raw(0),
        Vec::new(),
        vec![FrontEndModule::new(ModuleId::from_raw(0), source)],
    ));
    assert!(
        front_end.diagnostics().is_empty(),
        "{}",
        front_end.diagnostic_snapshot()
    );
    let symbol = front_end
        .hir()
        .expect("HIR")
        .functions()
        .iter()
        .find(|function| function.name() == function_name)
        .expect("named function")
        .symbol();
    let mir = lower_hir_bubble(front_end.hir().expect("HIR"), front_end.types()).expect("MIR");
    (mir, front_end.types().clone(), symbol)
}

fn executable_modules(texts: &[(&str, &str)]) -> (pop_mir::MirBubble, pop_types::TypeArena) {
    let modules = texts
        .iter()
        .enumerate()
        .map(|(index, (path, text))| {
            let raw = u32::try_from(index).expect("test Module count");
            FrontEndModule::new(
                ModuleId::from_raw(raw),
                SourceFile::new(FileId::from_raw(raw), *path, *text).expect("source"),
            )
        })
        .collect();
    let front_end = analyze_bubble(FrontEndBubbleInput::new(
        BubbleId::from_raw(0),
        NamespaceId::from_raw(0),
        Vec::new(),
        modules,
    ));
    assert!(
        front_end.diagnostics().is_empty(),
        "{}",
        front_end.diagnostic_snapshot()
    );
    let mir = lower_hir_bubble(front_end.hir().expect("HIR"), front_end.types()).expect("MIR");
    (mir, front_end.types().clone())
}

fn trap(kind: TrapKind) -> ExecutionError {
    ExecutionError::Runtime(RuntimeFailure::Trap(Trap::new(kind)))
}

#[test]
fn async_tasks_stay_cold_until_await_and_resume_with_the_exact_completion() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private async function failIfStarted(): Int\n\
             return 1 / 0\n\
         end\n\
         public function leaveCold(): Int\n\
             local task = failIfStarted()\n\
             return 7\n\
         end\n\
         private async function load(value: Int): Int\n\
             return value\n\
         end\n\
         public async function consume(): Int\n\
             local retained = \"live\"\n\
             local value = await load(42)\n\
             print(retained)\n\
             return value\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified async MIR");

    assert_eq!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Ok(vec![MirValue::Integer(
            IntegerValue::parse_decimal("7", IntegerKind::Int64).expect("seven"),
        )])
    );
    assert_eq!(
        interpreter.call(SymbolId::from_raw(3), &[]),
        Ok(vec![MirValue::Integer(
            IntegerValue::parse_decimal("42", IntegerKind::Int64).expect("forty two"),
        )])
    );
}

#[test]
fn failed_async_task_does_not_poison_later_interpreter_work() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private async function fail(): Int\n\
             return 1 / 0\n\
         end\n\
         public async function observeFailure(): Int\n\
             return await fail()\n\
         end\n\
         private async function load(value: Int): Int\n\
             return value\n\
         end\n\
         public async function continueWorking(): Int\n\
             return await load(9)\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified async MIR");

    assert_eq!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Err(trap(TrapKind::DivisionByZero))
    );
    assert_eq!(
        interpreter.call(SymbolId::from_raw(3), &[]),
        Ok(vec![MirValue::Integer(
            IntegerValue::parse_decimal("9", IntegerKind::Int64).expect("nine"),
        )])
    );
}

#[test]
fn direct_calls_checked_arithmetic_and_both_cfg_branches_execute() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private function add(left: Int, right: Int): Int\n\
             return left + right\n\
         end\n\
         public function choose(left: Int, right: Int): Int\n\
             if left < right then\n\
                 return add(left, right)\n\
             else\n\
                 return right\n\
             end\n\
         end\n",
    );
    let choose = mir
        .functions()
        .iter()
        .find(|function| function.symbol().raw() == 1)
        .expect("choose")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(choose, &[int(2), int(3)])
            .expect("then branch"),
        vec![int(5)]
    );
    assert_eq!(
        interpreter
            .call(choose, &[int(5), int(3)])
            .expect("else branch"),
        vec![int(3)]
    );
}

#[test]
fn safe_ffi_pointer_presence_executes_without_dynamic_conversion() {
    let ffi = BubbleId::from_raw(20);
    let source = SourceFile::new(
        FileId::from_raw(0),
        "src/pointers.pop",
        "namespace Pointers\n\
         public function inspect(pointer: Ffi.Pointer<Int>): Boolean\n\
             local optional = Ffi.OptionalPointer.fromPointer(pointer)\n\
             local readOnly = Ffi.Pointer.readOnly(pointer)\n\
             local optionalReadOnly = Ffi.OptionalReadOnlyPointer.fromPointer(readOnly)\n\
             local absent = Ffi.OptionalReadOnlyPointer.none<<Int>>()\n\
             return Ffi.OptionalPointer.isPresent(optional) and Ffi.OptionalReadOnlyPointer.isPresent(optionalReadOnly) and not Ffi.OptionalReadOnlyPointer.isPresent(absent)\n\
         end\n",
    )
    .expect("source");
    let front_end = analyze_bubble(
        FrontEndBubbleInput::new(
            BubbleId::from_raw(0),
            NamespaceId::from_raw(0),
            vec![ffi],
            vec![FrontEndModule::new(ModuleId::from_raw(0), source)],
        )
        .with_ffi_dependency(ffi),
    );
    assert!(
        front_end.diagnostics().is_empty(),
        "{}",
        front_end.diagnostic_snapshot()
    );
    let inspect = front_end
        .hir()
        .expect("HIR")
        .functions()
        .iter()
        .find(|function| function.name() == "inspect")
        .expect("inspect")
        .symbol();
    let mir = lower_hir_bubble(front_end.hir().expect("HIR"), front_end.types()).expect("MIR");
    let interpreter = MirInterpreter::new(&mir, front_end.types()).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(
                inspect,
                &[MirValue::FfiPointer(
                    ForeignAddress::new(0x1234).expect("non-null foreign address"),
                )],
            )
            .expect("pointer inspection"),
        vec![MirValue::Boolean(true)]
    );
}

#[test]
fn checked_ffi_pointer_require_returns_exact_present_and_absent_results() {
    let ffi = BubbleId::from_raw(20);
    let source = SourceFile::new(
        FileId::from_raw(0),
        "src/requirePointer.pop",
        "namespace Pointers\n\
         public function requirePointer(pointer: Ffi.OptionalPointer<Int>): Result<Ffi.Pointer<Int>, Ffi.NullPointerError>\n\
             return Ffi.OptionalPointer.require(pointer)\n\
         end\n",
    )
    .expect("source");
    let front_end = analyze_bubble(
        FrontEndBubbleInput::new(
            BubbleId::from_raw(0),
            NamespaceId::from_raw(0),
            vec![ffi],
            vec![FrontEndModule::new(ModuleId::from_raw(0), source)],
        )
        .with_ffi_dependency(ffi),
    );
    assert!(
        front_end.diagnostics().is_empty(),
        "{}",
        front_end.diagnostic_snapshot()
    );
    let require = front_end
        .hir()
        .expect("HIR")
        .functions()
        .iter()
        .find(|function| function.name() == "requirePointer")
        .expect("requirePointer")
        .symbol();
    let mir = lower_hir_bubble(front_end.hir().expect("HIR"), front_end.types()).expect("MIR");
    let interpreter = MirInterpreter::new(&mir, front_end.types()).expect("verified MIR");
    let address = ForeignAddress::new(0x1234).expect("non-null foreign address");

    assert_eq!(
        interpreter
            .call(require, &[MirValue::FfiPointer(address)])
            .expect("present pointer"),
        vec![MirValue::Result {
            definition: BuiltinTypeId::from_raw(100),
            case: ResultCaseId::from_raw(0),
            arguments: vec![MirValue::FfiPointer(address)],
        }]
    );
    assert_eq!(
        interpreter
            .call(require, &[MirValue::Nil])
            .expect("absent pointer"),
        vec![MirValue::Result {
            definition: BuiltinTypeId::from_raw(100),
            case: ResultCaseId::from_raw(1),
            arguments: vec![MirValue::FfiNullPointerError],
        }]
    );
}

#[test]
fn completed_async_tasks_execute_through_await() {
    let (mir, types, run) = executable_source_function(
        "namespace Main\n\
         private async function load(): Int\n\
             return 42\n\
         end\n\
         public async function run(): Int\n\
             local pending = load()\n\
             return await pending\n\
        end\n",
        "run",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(interpreter.call(run, &[]).expect("await"), vec![int(42)]);
}

#[test]
fn async_cleanup_awaits_before_the_enclosing_task_completes() {
    let (mir, types, run) = executable_source_function(
        "namespace Main\n\
         private async function failDuringClose(): Int\n\
             return 1 / 0\n\
         end\n\
         public async function run(): Int\n\
             async defer\n\
                 local ignored = await failDuringClose()\n\
             end\n\
             return 7\n\
         end\n",
        "run",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified async cleanup MIR");

    assert_eq!(
        interpreter.call(run, &[]),
        Err(trap(TrapKind::DivisionByZero))
    );
}

#[test]
fn structured_group_transfers_child_ownership_and_returns_the_exact_completion() {
    let (mir, types, run) = executable_source_function(
        "namespace Main\n\
         private async function load(cancel: CancelToken): Int\n\
             return 42\n\
         end\n\
         public async function run(): Int\n\
             local source = Task.cancellationSource()\n\
             local cancel = Task.cancelToken(source)\n\
             local grouped = Task.group(cancel, async function(group: Task.Group): Int\n\
                 local child = Task.start(group, load(cancel))\n\
                 return await child\n\
             end)\n\
             return await grouped\n\
         end\n",
        "run",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified structured-task MIR");

    assert_eq!(
        interpreter.call(run, &[]).expect("group completion"),
        vec![int(42)]
    );
}

#[test]
fn closing_group_joins_an_unawaited_child_and_propagates_its_failure() {
    let (mir, types, run) = executable_source_function(
        "namespace Main\n\
         private async function fail(cancel: CancelToken): Int\n\
             return 1 / 0\n\
         end\n\
         public async function run(): Int\n\
             local source = Task.cancellationSource()\n\
             local cancel = Task.cancelToken(source)\n\
             local grouped = Task.group(cancel, async function(group: Task.Group): Int\n\
                 local ignored = Task.start(group, fail(cancel))\n\
                 return 7\n\
             end)\n\
             return await grouped\n\
         end\n",
        "run",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified structured-task MIR");

    assert_eq!(
        interpreter.call(run, &[]),
        Err(trap(TrapKind::DivisionByZero))
    );
}

#[test]
fn explicit_cancellation_is_observed_but_async_cleanup_await_is_masked() {
    let (mir, types, run) = executable_source_function(
        "namespace Main\n\
         private async function pending(cancel: CancelToken): Int\n\
             return 8\n\
         end\n\
         private async function failDuringCleanup(): Int\n\
             return 1 / 0\n\
         end\n\
         public async function run(): Int\n\
             local source = Task.cancellationSource()\n\
             local cancel = Task.cancelToken(source)\n\
             local grouped = Task.group(cancel, async function(group: Task.Group): Int\n\
                 async defer\n\
                     local ignored = await failDuringCleanup()\n\
                 end\n\
                 local child = Task.start(group, pending(cancel))\n\
                 return await child\n\
             end)\n\
             Task.cancel(source)\n\
             return await grouped\n\
         end\n",
        "run",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified cancellation MIR");

    assert_eq!(
        interpreter.call(run, &[]),
        Err(trap(TrapKind::DivisionByZero))
    );
}

#[test]
fn portable_cross_bubble_generic_capsules_execute_private_helpers() {
    let library_bubble = BubbleId::from_raw(2);
    let library_source = SourceFile::new(
        FileId::from_raw(0),
        "src/generics.pop",
        "namespace Pop.Sequence\n\
         private function privateIdentity<T>(value: T): T\n\
             return value\n\
         end\n\
         public function portableIdentity<T>(value: T): T\n\
             return privateIdentity(value)\n\
         end\n",
    )
    .expect("library source");
    let library = analyze_bubble(FrontEndBubbleInput::new(
        library_bubble,
        NamespaceId::from_raw(2),
        Vec::new(),
        vec![FrontEndModule::new(ModuleId::from_raw(0), library_source)],
    ));
    assert!(library.diagnostics().is_empty());
    let metadata = library
        .reference_metadata()
        .expect("portable metadata")
        .clone();

    let application_source = SourceFile::new(
        FileId::from_raw(1),
        "src/main.pop",
        "namespace Application\n\
         using Pop.Sequence\n\
         public function run(): Int\n\
             return portableIdentity(42)\n\
         end\n",
    )
    .expect("application source");
    let application = analyze_bubble(
        FrontEndBubbleInput::new(
            BubbleId::from_raw(7),
            NamespaceId::from_raw(7),
            vec![library_bubble],
            vec![FrontEndModule::new(
                ModuleId::from_raw(1),
                application_source,
            )],
        )
        .with_reference_metadata(vec![metadata]),
    );
    assert!(
        application.diagnostics().is_empty(),
        "{}",
        application.diagnostic_snapshot()
    );
    let hir = application.hir().expect("consumer HIR");
    let entry = hir
        .functions()
        .iter()
        .find(|function| function.name() == "run")
        .expect("entry")
        .symbol();
    let mir = lower_hir_bubble(hir, application.types()).expect("specialized MIR");
    let interpreter = MirInterpreter::new(&mir, application.types()).expect("verified MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("capsule call"),
        vec![int(42)]
    );
}

#[test]
fn generalized_iteration_executes_arrays_and_table_tuple_bindings_in_order() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function sum(values: {Int}): Int\n\
             local total = 0\n\
             for value in values do\n\
                 total = total + value\n\
             end\n\
             return total\n\
         end\n\
         public function sumTable(entries: {[String]: Int}): Int\n\
             local total = 0\n\
             for key, value in entries do\n\
                 if key == \"first\" then\n\
                     total = total + value\n\
                 else\n\
                     total = total + value\n\
                 end\n\
             end\n\
             return total\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified iteration MIR");

    assert_eq!(
        interpreter
            .call(
                mir.functions()[0].symbol(),
                &[MirValue::Array(vec![int(2), int(3), int(5)])],
            )
            .expect("array iteration"),
        vec![int(10)]
    );
    assert_eq!(
        interpreter
            .call(
                mir.functions()[1].symbol(),
                &[MirValue::Table(vec![
                    (MirValue::String("first".to_owned()), int(7)),
                    (MirValue::String("second".to_owned()), int(11)),
                ])],
            )
            .expect("table iteration"),
        vec![int(18)]
    );
}

#[test]
fn string_iteration_decodes_each_unicode_scalar_once_in_order() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function inspect(): Int\n\
             local count = 0\n\
             for rune in \"Aé中😀\\0e\\u{301}\" do\n\
                 count += 1\n\
                 local point = Unicode.codePoint(rune)\n\
                 if count == 1 and point ~= 65 then\n\
                     return 1\n\
                 end\n\
                 if count == 2 and point ~= 233 then\n\
                     return 2\n\
                 end\n\
                 if count == 3 and point ~= 20013 then\n\
                     return 3\n\
                 end\n\
                 if count == 4 and point ~= 128512 then\n\
                     return 4\n\
                 end\n\
                 if count == 5 and point ~= 0 then\n\
                     return 5\n\
                 end\n\
                 if count == 6 and point ~= 101 then\n\
                     return 6\n\
                 end\n\
                 if count == 7 and point ~= 769 then\n\
                     return 7\n\
                 end\n\
             end\n\
             if count ~= 7 then\n\
                 return 8\n\
             end\n\
             local emptyCount = 0\n\
             for rune in \"\" do\n\
                 emptyCount += 1\n\
             end\n\
             return 42 + emptyCount\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified String iteration MIR")
            .call(function, &[])
            .expect("String iteration"),
        vec![int(42)]
    );
}

#[test]
fn generalized_iteration_observes_replacement_and_traps_structural_mutation() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function replaceDuringIteration(): Int\n\
             local entries: {[String]: Int} = { first = 1, second = 2 }\n\
             local total = 0\n\
             for key, value in entries do\n\
                 if key == \"first\" then\n\
                     entries[\"second\"] = 9\n\
                 end\n\
                 total = total + value\n\
             end\n\
             return total\n\
         end\n\
         public function growDuringIteration(): Int\n\
             local entries: {[String]: Int} = { first = 1 }\n\
             for key, value in entries do\n\
                 if key == \"first\" then\n\
                     entries[\"second\"] = value\n\
                 end\n\
             end\n\
             return 0\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified mutation MIR");

    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[])
            .expect("replacement remains visible"),
        vec![int(10)]
    );
    assert_eq!(
        interpreter.call(mir.functions()[1].symbol(), &[]),
        Err(trap(TrapKind::ConcurrentModification))
    );
}

#[test]
fn ordinary_pop_sequence_adapters_are_lazy_ordered_and_materialize_on_demand() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             public function sequenceResult(): Int\n\
                 local calls = 0\n\
                 local values: {Int} = {1, 2, 3}\n\
                 local mapped = map(values, function(value: Int): Int\n\
                     return value\n\
                 end)\n\
                 if calls ~= 0 then\n\
                     return -1\n\
                 end\n\
                 local filtered = filter(mapped, function(value: Int): Boolean\n\
                     return value > 1\n\
                 end)\n\
                 local collected = collect(filtered)\n\
                 return List.get(collected, 1) * 10 + List.get(collected, 2)\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("sequenceResult")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Sequence MIR");
    assert_eq!(
        interpreter.call(function, &[]).expect("Sequence execution"),
        vec![int(23)]
    );
}

#[test]
fn ordinary_pop_sequence_aggregates_short_circuit_without_materializing() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             public function aggregateResult(): Int\n\
                 local values: {Int} = {1, 2, 3, 4}\n\
                 local found = any(values, function(value: Int): Boolean\n\
                     return value > 2\n\
                 end)\n\
                 local matched = all(values, function(value: Int): Boolean\n\
                     return value < 3\n\
                 end)\n\
                 local empty: {Int} = {}\n\
                 if not found or matched or any(empty, function(value: Int): Boolean\n\
                     return true\n\
                 end) or not all(empty, function(value: Int): Boolean\n\
                     return false\n\
                 end) then\n\
                     return -1\n\
                 end\n\
                 return count(values)\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("aggregateResult")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Sequence MIR");
    assert_eq!(
        interpreter
            .call(function, &[])
            .expect("Sequence aggregates"),
        vec![int(4)]
    );
}

#[test]
fn ordinary_pop_sequence_inspection_and_visitation_are_direct() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             private function valueOr(step: Iteration<Int>, fallback: Int): Int\n\
                 match step\n\
                 when Iteration.Item(value) then\n\
                     return value\n\
                 when Iteration.End then\n\
                     return fallback\n\
                 end\n\
             end\n\
             private function hasItem<T>(step: Iteration<T>): Boolean\n\
                 match step\n\
                 when Iteration.Item(value) then\n\
                     return true\n\
                 when Iteration.End then\n\
                     return false\n\
                 end\n\
             end\n\
             public function terminalResult(): Int\n\
                 local empty: {Int} = {}\n\
                 local single: {Int} = {9}\n\
                 local values: {Int} = {1, 2, 3, 4}\n\
                 local absent: Int? = empty[1]\n\
                 local optionalValues: {Int?} = {absent}\n\
                 if not hasItem(first(optionalValues)) or hasItem(first(empty)) then\n\
                     return -1\n\
                 end\n\
                 if not isEmpty(empty) or isEmpty(values) then\n\
                     return -1\n\
                 end\n\
                 if firstOr(optionalValues, absent) ~= nil then\n\
                     return -1\n\
                 end\n\
                 each(values, function(value: Int)\n\
                     value\n\
                 end)\n\
                 local matches = countWhere(values, function(value: Int): Boolean\n\
                     return value == 2 or value == 4\n\
                 end)\n\
                 if not none(values, function(value: Int): Boolean\n\
                     return value > 4\n\
                 end) then\n\
                     return -1\n\
                 end\n\
                 local noEven = none(values, function(value: Int): Boolean\n\
                     return value == 2\n\
                 end)\n\
                 if noEven then\n\
                     return -1\n\
                 end\n\
                 return firstOr(values, 20) + lastOr(values, 20) * 2 + firstOr(empty, 7) + lastOr(empty, 8) + firstOr(single, 0) + lastOr(single, 0) + matches + valueOr(first(values), 0) + valueOr(last(values), 0) + valueOr(first(empty), 1)\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("terminalResult")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified Sequence terminal MIR")
            .call(function, &[])
            .expect("Sequence inspection and visitation"),
        vec![int(50)]
    );
}

#[test]
fn ordinary_pop_integer_sequence_aggregates_are_checked_and_explicit() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             public function aggregateNumbers(mode: Int): Int\n\
                 local empty: {Int} = {}\n\
                 local values: {Int} = {2, 3, 4}\n\
                 if mode == 0 then\n\
                     return sum(values) + product(values) + minOr(values, 100) + maxOr(values, -100) + sum(empty) + product(empty) + minOr(empty, 7) + maxOr(empty, 8)\n\
                 end\n\
                 local overflow: {Int} = {9223372036854775807, 1}\n\
                 if mode == 1 then\n\
                     return sum(overflow)\n\
                 end\n\
                 local productOverflow: {Int} = {9223372036854775807, 2}\n\
                 if mode == 2 then\n\
                     return product(productOverflow)\n\
                 end\n\
                 if mode == 3 then\n\
                     return sumBy(overflow, function(value: Int): Int\n\
                         return value\n\
                     end)\n\
                 end\n\
                 return productBy(productOverflow, function(value: Int): Int\n\
                     return value\n\
                 end)\n\
             end\n",
        ),
    ]);
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Sequence numeric MIR");
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().len() == 1)
        .expect("aggregateNumbers")
        .symbol();
    assert_eq!(interpreter.call(function, &[int(0)]), Ok(vec![int(55)]));
    assert_eq!(
        interpreter.call(function, &[int(1)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter.call(function, &[int(2)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter.call(function, &[int(3)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter.call(function, &[int(4)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
}

#[test]
fn sequence_projections_are_exact_stable_and_generic() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             private record Candidate\n\
                 id: Int\n\
                 key: Int\n\
             end\n\
             public function projectionContract(): Int\n\
                 local first: Candidate = { id = 1, key = 5 }\n\
                 local second: Candidate = { id = 2, key = 5 }\n\
                 local third: Candidate = { id = 3, key = 7 }\n\
                 local fourth: Candidate = { id = 4, key = 7 }\n\
                 local candidates: {Candidate} = {first, second, third, fourth}\n\
                 local least = minByOr(candidates, function(value: Candidate): Int\n\
                     return value.key\n\
                 end, third)\n\
                 local greatest = maxByOr(candidates, function(value: Candidate): Int\n\
                     return value.key\n\
                 end, first)\n\
                 local values: {Int} = {1, 2, 3}\n\
                 local total = sumBy(values, function(value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local multiplied = productBy(values, function(value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local words: {String} = {\"first\", \"match\", \"last\"}\n\
                 local word = findOr(words, function(value: String): Boolean\n\
                     return value == \"match\"\n\
                 end, \"missing\")\n\
                 if least.id ~= 1 or greatest.id ~= 3 then\n\
                     return -1\n\
                 end\n\
                 if total ~= 6 or multiplied ~= 6 then\n\
                     return -2\n\
                 end\n\
                 if word ~= \"match\" then\n\
                     return -3\n\
                 end\n\
                 return 0\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("projectionContract")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified projection MIR")
            .call(function, &[]),
        Ok(vec![int(0)])
    );
}

#[test]
fn sequence_append_prepend_and_scan_are_lazy_and_stably_exhausted() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             private class CountingIterator implements Iterator<Int>\n\
                 private current: Int\n\
                 private limit: Int\n\
                 private calls: Int\n\
                 public function CountingIterator.new(limit: Int): CountingIterator\n\
                     return CountingIterator { current = 0, limit = limit, calls = 0 }\n\
                 end\n\
                 public function CountingIterator:iterator(): Iterator<Int>\n\
                     return self\n\
                 end\n\
                 public function CountingIterator:next(): Iteration<Int>\n\
                     self.calls += 1\n\
                     if self.current >= self.limit then\n\
                         return Iteration.End\n\
                     end\n\
                     self.current += 1\n\
                     return Iteration.Item(self.current)\n\
                 end\n\
                 public function CountingIterator:callCount(): Int\n\
                     return self.calls\n\
                 end\n\
             end\n\
             public function lazyContract(): Int\n\
                 local appendCounter = CountingIterator.new(2)\n\
                 local appendSource: Iterator<Int> = appendCounter\n\
                 local appended = append(appendSource, 9)\n\
                 if appendCounter:callCount() ~= 0 then\n\
                     return -1\n\
                 end\n\
                 if count(take(appended, 1)) ~= 1 or appendCounter:callCount() ~= 1 then\n\
                     return -2\n\
                 end\n\
                 if count(appended) ~= 2 or appendCounter:callCount() ~= 3 then\n\
                     return -3\n\
                 end\n\
                 if count(appended) ~= 0 or appendCounter:callCount() ~= 3 then\n\
                     return -4\n\
                 end\n\
                 local prependCounter = CountingIterator.new(2)\n\
                 local prependSource: Iterator<Int> = prependCounter\n\
                 local prepended = prepend(prependSource, 9)\n\
                 if prependCounter:callCount() ~= 0 then\n\
                     return -5\n\
                 end\n\
                 if count(take(prepended, 1)) ~= 1 or prependCounter:callCount() ~= 0 then\n\
                     return -6\n\
                 end\n\
                 if count(prepended) ~= 2 or prependCounter:callCount() ~= 3 then\n\
                     return -7\n\
                 end\n\
                 if count(prepended) ~= 0 or prependCounter:callCount() ~= 3 then\n\
                     return -8\n\
                 end\n\
                 local scanCounter = CountingIterator.new(2)\n\
                 local scanSource: Iterator<Int> = scanCounter\n\
                 local scanned = scan(scanSource, 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 if scanCounter:callCount() ~= 0 then\n\
                     return -9\n\
                 end\n\
                 if count(take(scanned, 1)) ~= 1 or scanCounter:callCount() ~= 1 then\n\
                     return -10\n\
                 end\n\
                 if count(scanned) ~= 1 or scanCounter:callCount() ~= 3 then\n\
                     return -11\n\
                 end\n\
                 if count(scanned) ~= 0 or scanCounter:callCount() ~= 3 then\n\
                     return -12\n\
                 end\n\
                 return 0\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("lazyContract")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified lazy Sequence MIR")
            .call(function, &[]),
        Ok(vec![int(0)])
    );
}

#[test]
fn exact_source_overloads_execute_in_the_mir_interpreter() {
    let (mir, types) = executable_modules(&[
        (
            "src/int.pop",
            "namespace Main\npublic function choose(value: Int): Int return value + 1 end\n",
        ),
        (
            "src/text.pop",
            "namespace Main\npublic function choose(value: String): String return value .. \"!\" end\n",
        ),
        (
            "src/main.pop",
            "namespace Main\npublic function overloadResult(): Int\n    if choose(\"pop\") ~= \"pop!\" then\n        return -1\n    end\n    return choose(41)\nend\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("overloadResult")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified overload MIR")
            .call(function, &[]),
        Ok(vec![int(42)])
    );
}

#[test]
fn sequence_index_last_and_reduction_are_generic_and_exact() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            include_str!(
                "../../../../libraries/standard/tests/programs/sequenceIndexLastReduction.pop"
            ),
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("main")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified Sequence inspection MIR")
            .call(function, &[]),
        Ok(vec![int(0)])
    );
}

#[test]
fn ordinary_pop_sequence_projection_and_composition_are_direct() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             public function projectedResult(): Int\n\
                 local values: {Int} = {3, 1, 2}\n\
                 local empty: {Int} = {}\n\
                 local selected = findOr(values, function(value: Int): Boolean\n\
                     return value == 2\n\
                 end, 9)\n\
                 local position = indexOr(values, function(value: Int): Boolean\n\
                     return value == 2\n\
                 end, -1)\n\
                 local projectedSum = sumBy(values, function(value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local projectedProduct = productBy(values, function(value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local least = minByOr(values, function(value: Int): Int\n\
                     return value\n\
                 end, 9)\n\
                 local greatest = maxByOr(values, function(value: Int): Int\n\
                     return value\n\
                 end, 9)\n\
                 local appended = collect(append(values, 9))\n\
                 local prepended = collect(prepend(values, 8))\n\
                 local states = scan(values, 10, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 if findOr(empty, function(value: Int): Boolean\n\
                     return true\n\
                 end, 7) ~= 7 or indexOr(empty, function(value: Int): Boolean\n\
                     return true\n\
                 end, -4) ~= -4 then\n\
                     return -1\n\
                 end\n\
                 return selected + position + projectedSum + projectedProduct + least + greatest + List.get(appended, 4) + List.get(prepended, 1) + sum(states)\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("projectedResult")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified projected Sequence MIR")
            .call(function, &[])
            .expect("Sequence projection and composition"),
        vec![int(44)]
    );
}

#[test]
fn ordinary_pop_lazy_sequence_bounds_and_composition_preserve_state() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             public function boundedResult(): Int\n\
                 local empty: {Int} = {}\n\
                 local single: {Int} = {9}\n\
                 local values: {Int} = {1, 2, 3, 4, 5}\n\
                 if count(take(values, -1)) ~= 0 or count(take(values, 0)) ~= 0 or count(take(values, 10)) ~= 5 then\n\
                     return -1\n\
                 end\n\
                 if count(drop(values, -1)) ~= 5 or count(drop(values, 10)) ~= 0 then\n\
                     return -1\n\
                 end\n\
                 local prefix = takeWhile(values, function(value: Int): Boolean\n\
                     return value < 4\n\
                 end)\n\
                 local prefixSum = fold(prefix, 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local suffix = dropWhile(values, function(value: Int): Boolean\n\
                     return value < 3\n\
                 end)\n\
                 local suffixSum = fold(suffix, 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 if count(prefix) ~= 0 then\n\
                     return -1\n\
                 end\n\
                 local takeSum = fold(take(values, 3), 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local dropSum = fold(drop(values, 2), 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local joinedSum = fold(concat(take(values, 2), drop(values, 3)), 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 local edgeSum = fold(concat(empty, single), 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end) + fold(concat(single, empty), 0, function(state: Int, value: Int): Int\n\
                     return value\n\
                 end)\n\
                 return takeSum + dropSum + prefixSum + suffixSum + joinedSum + edgeSum\n\
             end\n",
        ),
    ]);
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("boundedResult")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified lazy Sequence MIR")
            .call(function, &[])
            .expect("lazy Sequence bounds and composition"),
        vec![int(39)]
    );
}

#[allow(clippy::too_many_lines)]
#[test]
fn ordinary_pop_integer_math_is_portable_and_checked() {
    let (mir, types) = executable_modules(&[(
        "src/math.pop",
        include_str!("../../../../libraries/standard/pop/src/math.pop"),
    )]);
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Math MIR");
    let [
        minimum,
        maximum,
        absolute,
        divisor,
        sign,
        multiple,
        coprime,
        clamp,
        power,
        floor_divide,
        floor_remainder,
    ] = mir.functions()
    else {
        panic!("Math source must contain exactly eleven functions");
    };

    assert_eq!(
        interpreter.call(minimum.symbol(), &[int(7), int(3)]),
        Ok(vec![int(3)])
    );
    assert_eq!(
        interpreter.call(maximum.symbol(), &[int(-2), int(5)]),
        Ok(vec![int(5)])
    );
    assert_eq!(
        interpreter.call(absolute.symbol(), &[int(-4)]),
        Ok(vec![int(4)])
    );
    assert_eq!(
        interpreter.call(divisor.symbol(), &[int(54), int(-24)]),
        Ok(vec![int(6)])
    );
    assert_eq!(
        interpreter.call(divisor.symbol(), &[int(0), int(0)]),
        Ok(vec![int(0)])
    );
    assert_eq!(
        interpreter.call(divisor.symbol(), &[int(-54), int(24)]),
        Ok(vec![int(6)])
    );
    assert_eq!(
        interpreter.call(divisor.symbol(), &[int(13), int(17)]),
        Ok(vec![int(1)])
    );
    assert_eq!(
        interpreter.call(divisor.symbol(), &[int(24), int(54)]),
        Ok(vec![int(6)])
    );
    assert_eq!(
        interpreter.call(absolute.symbol(), &[int(i64::MIN)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter.call(sign.symbol(), &[int(-20)]),
        Ok(vec![int(-1)])
    );
    assert_eq!(interpreter.call(sign.symbol(), &[int(0)]), Ok(vec![int(0)]));
    assert_eq!(
        interpreter.call(sign.symbol(), &[int(i64::MIN)]),
        Ok(vec![int(-1)])
    );
    assert_eq!(
        interpreter.call(multiple.symbol(), &[int(21), int(-6)]),
        Ok(vec![int(42)])
    );
    assert_eq!(
        interpreter.call(multiple.symbol(), &[int(i64::MIN), int(0)]),
        Ok(vec![int(0)])
    );
    assert_eq!(
        interpreter.call(multiple.symbol(), &[int(3_000_000_000), int(6_000_000_000)]),
        Ok(vec![int(6_000_000_000)])
    );
    assert_eq!(
        interpreter.call(multiple.symbol(), &[int(i64::MAX), int(2)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter.call(coprime.symbol(), &[int(35), int(64)]),
        Ok(vec![MirValue::Boolean(true)])
    );
    assert_eq!(
        interpreter.call(coprime.symbol(), &[int(21), int(6)]),
        Ok(vec![MirValue::Boolean(false)])
    );
    assert_eq!(
        interpreter.call(clamp.symbol(), &[int(-4), int(-2), int(8)]),
        Ok(vec![int(-2)])
    );
    assert_eq!(
        interpreter.call(clamp.symbol(), &[int(4), int(-2), int(8)]),
        Ok(vec![int(4)])
    );
    assert_eq!(
        interpreter.call(clamp.symbol(), &[int(-2), int(-2), int(8)]),
        Ok(vec![int(-2)])
    );
    assert_eq!(
        interpreter.call(clamp.symbol(), &[int(8), int(-2), int(8)]),
        Ok(vec![int(8)])
    );
    assert_eq!(
        interpreter.call(clamp.symbol(), &[int(9), int(-2), int(8)]),
        Ok(vec![int(8)])
    );
    assert_eq!(
        interpreter.call(clamp.symbol(), &[int(4), int(8), int(-2)]),
        Ok(vec![MirValue::Nil])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(2), int(10)]),
        Ok(vec![int(1_024)])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(3), int(5)]),
        Ok(vec![int(243)])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(0), int(0)]),
        Ok(vec![int(1)])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(0), int(1)]),
        Ok(vec![int(0)])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(i64::MAX), int(1)]),
        Ok(vec![int(i64::MAX)])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(2), int(-1)]),
        Ok(vec![MirValue::Nil])
    );
    assert_eq!(
        interpreter.call(power.symbol(), &[int(2), int(63)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    for (dividend, divisor, quotient, remainder) in [
        (7, 3, 2, 1),
        (6, 3, 2, 0),
        (-7, 3, -3, 2),
        (7, -3, -3, -2),
        (-7, -3, 2, -1),
        (2, 3, 0, 2),
    ] {
        assert_eq!(
            interpreter.call(floor_divide.symbol(), &[int(dividend), int(divisor)]),
            Ok(vec![int(quotient)])
        );
        assert_eq!(
            interpreter.call(floor_remainder.symbol(), &[int(dividend), int(divisor)]),
            Ok(vec![int(remainder)])
        );
    }
    assert_eq!(
        interpreter.call(floor_divide.symbol(), &[int(1), int(0)]),
        Err(trap(TrapKind::DivisionByZero))
    );
    assert_eq!(
        interpreter.call(floor_remainder.symbol(), &[int(1), int(0)]),
        Err(trap(TrapKind::DivisionByZero))
    );
    assert_eq!(
        interpreter.call(floor_divide.symbol(), &[int(i64::MIN), int(-1)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter.call(floor_remainder.symbol(), &[int(i64::MIN), int(-1)]),
        Err(trap(TrapKind::IntegerOverflow))
    );
}

#[test]
fn ordinary_pop_bytes_inspection_and_endian_reads_are_portable() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("backend crate is under the repository root");
    let bytes_source =
        std::fs::read_to_string(root.join("crates/libraries/standard/pop/src/bytes.pop"))
            .expect("read Pop.Bytes source");
    let (mir, types) = executable_modules(&[
        ("src/bytes.pop", bytes_source.as_str()),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Bytes\n\
             public function inspect(value: Bytes, equalValue: Bytes, prefix: Bytes, suffix: Bytes, empty: Bytes, maximum: Bytes): Int\n\
                 local view = Bytes.view(value)\n\
                 local equalView = Bytes.view(equalValue)\n\
                 local prefixView = Bytes.view(prefix)\n\
                 local suffixView = Bytes.view(suffix)\n\
                 local emptyView = Bytes.view(empty)\n\
                 local maximumView = Bytes.view(maximum)\n\
                 if not equals(view, equalView) or compare(view, equalView) ~= 0 then\n\
                     return 1\n\
                 end\n\
                 if compare(prefixView, view) ~= -1 or compare(view, prefixView) ~= 1 then\n\
                     return 2\n\
                 end\n\
                 if not startsWith(view, prefixView) or not endsWith(view, suffixView) then\n\
                     return 3\n\
                 end\n\
                 if not contains(view, 255) or (indexOf(view, 255, 1) ?? 0) ~= 5 then\n\
                     return 4\n\
                 end\n\
                 if indexOf(view, 1, 0) ~= nil or indexOf(view, 7, 1) ~= nil then\n\
                     return 5\n\
                 end\n\
                 if (readUInt16BigEndian(view, 1) ?? 0) ~= 258 or (readUInt16LittleEndian(view, 1) ?? 0) ~= 513 then\n\
                     return 6\n\
                 end\n\
                 if (readUInt32BigEndian(view, 1) ?? 0) ~= 16909060 or (readUInt32LittleEndian(view, 1) ?? 0) ~= 67305985 then\n\
                     return 7\n\
                 end\n\
                 if (readUInt64BigEndian(view, 1) ?? 0) ~= 72623863984324672 or (readUInt64LittleEndian(view, 1) ?? 0) ~= 4647715910730318337 then\n\
                     return 8\n\
                 end\n\
                 if readUInt16BigEndian(view, 8) ~= nil or readUInt64LittleEndian(view, 2) ~= nil then\n\
                     return 9\n\
                 end\n\
                 if not equals(emptyView, emptyView) or compare(emptyView, prefixView) ~= -1 or not startsWith(view, emptyView) or not endsWith(view, emptyView) then\n\
                     return 10\n\
                 end\n\
                 if contains(emptyView, 0) or indexOf(emptyView, 0, 1) ~= nil or readUInt16BigEndian(view, -1) ~= nil or readUInt16BigEndian(view, 9223372036854775807) ~= nil then\n\
                     return 11\n\
                 end\n\
                 if (readUInt16BigEndian(view, 2) ?? 0) ~= 515 or (readUInt16LittleEndian(view, 2) ?? 0) ~= 770 then\n\
                     return 12\n\
                 end\n\
                 if (readUInt16BigEndian(maximumView, 1) ?? 0) ~= 65535 or (readUInt32LittleEndian(maximumView, 1) ?? 0) ~= 4294967295 or (readUInt64BigEndian(maximumView, 1) ?? 0) ~= 18446744073709551615 then\n\
                     return 13\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Bytes consumer").symbol();
    let mut runtime = GenerationalRuntime::new();
    let value = runtime
        .allocate_immutable_bytes(&[1, 2, 3, 4, 255, 0, 128, 64])
        .expect("value Bytes");
    let equal = runtime
        .allocate_immutable_bytes(&[1, 2, 3, 4, 255, 0, 128, 64])
        .expect("equal Bytes");
    let prefix = runtime
        .allocate_immutable_bytes(&[1, 2])
        .expect("prefix Bytes");
    let suffix = runtime
        .allocate_immutable_bytes(&[128, 64])
        .expect("suffix Bytes");
    let empty = runtime.allocate_immutable_bytes(&[]).expect("empty Bytes");
    let maximum = runtime
        .allocate_immutable_bytes(&[255, 255, 255, 255, 255, 255, 255, 255])
        .expect("maximum Bytes");
    let interpreter =
        MirInterpreter::with_runtime(&mir, &types, runtime).expect("verified Bytes MIR");

    assert_eq!(
        interpreter
            .call(
                entry,
                &[
                    MirValue::Bytes(value),
                    MirValue::Bytes(equal),
                    MirValue::Bytes(prefix),
                    MirValue::Bytes(suffix),
                    MirValue::Bytes(empty),
                    MirValue::Bytes(maximum),
                ],
            )
            .expect("portable Bytes execution"),
        vec![int(42)]
    );
}

#[test]
fn reusable_byte_buffers_preserve_order_endianness_and_snapshot_independence() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function verify(): Int\n\
             local buffer = Bytes.withCapacity(2)\n\
             Bytes.reserve(buffer, 32)\n\
             Bytes.write(buffer, 170)\n\
             Bytes.writeUInt16BigEndian(buffer, 258)\n\
             Bytes.writeUInt16LittleEndian(buffer, 772)\n\
             Bytes.writeUInt32BigEndian(buffer, 84281096)\n\
             Bytes.writeUInt64LittleEndian(buffer, 72623859790382856)\n\
             if Bytes.length(buffer) ~= 17 then\n\
                 return 1\n\
             end\n\
             local snapshot = Bytes.toBytes(buffer)\n\
             Bytes.clear(buffer)\n\
             Bytes.write(buffer, 9)\n\
             local current = Bytes.toBytes(buffer)\n\
             if Bytes.length(Bytes.view(snapshot)) ~= 17 or (Bytes.get(Bytes.view(snapshot), 1) ?? 0) ~= 170 then\n\
                 return 2\n\
             end\n\
             if Bytes.length(Bytes.view(current)) ~= 1 or (Bytes.get(Bytes.view(current), 1) ?? 0) ~= 9 then\n\
                 return 3\n\
             end\n\
             local combined = Bytes.create()\n\
             Bytes.write(combined, snapshot)\n\
             Bytes.write(combined, Bytes.slice(snapshot, 2, 2))\n\
             local result = Bytes.toBytes(combined)\n\
             if Bytes.length(Bytes.view(result)) ~= 19 then\n\
                 return 4\n\
             end\n\
             if (Bytes.get(Bytes.view(result), 2) ?? 0) ~= 1 or (Bytes.get(Bytes.view(result), 3) ?? 0) ~= 2 then\n\
                 return 5\n\
             end\n\
             if (Bytes.get(Bytes.view(result), 4) ?? 0) ~= 4 or (Bytes.get(Bytes.view(result), 5) ?? 0) ~= 3 then\n\
                 return 6\n\
             end\n\
             if (Bytes.get(Bytes.view(result), 6) ?? 0) ~= 5 or (Bytes.get(Bytes.view(result), 9) ?? 0) ~= 8 then\n\
                 return 7\n\
             end\n\
             if (Bytes.get(Bytes.view(result), 10) ?? 0) ~= 8 or (Bytes.get(Bytes.view(result), 17) ?? 0) ~= 1 then\n\
                 return 8\n\
             end\n\
             if (Bytes.get(Bytes.view(result), 18) ?? 0) ~= 1 or (Bytes.get(Bytes.view(result), 19) ?? 0) ~= 2 then\n\
                 return 9\n\
             end\n\
             return 42\n\
         end\n",
    );
    let entry = mir
        .functions()
        .last()
        .expect("byte-buffer function")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified byte-buffer MIR");

    assert_eq!(
        interpreter.call(entry, &[]).expect("byte-buffer execution"),
        vec![int(42)]
    );
}

#[test]
fn reusable_byte_buffers_trap_before_negative_capacity_mutation() {
    for source in [
        "namespace Main\npublic function fail(): Int\nlocal buffer = Bytes.withCapacity(-1)\nreturn 0\nend\n",
        "namespace Main\npublic function fail(): Int\nlocal buffer = Bytes.create()\nBytes.reserve(buffer, -1)\nreturn 0\nend\n",
    ] {
        let (mir, types) = executable_source(source);
        let function = mir.functions()[0].symbol();
        assert_eq!(
            MirInterpreter::new(&mir, &types)
                .expect("verified byte-buffer MIR")
                .call(function, &[]),
            Err(trap(TrapKind::BoundsViolation))
        );
    }
}

#[test]
fn checked_utf8_transcoding_is_exact_and_keeps_buffers_reusable() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function verify(): Int\n\
             local text = \"Aé中🦀\"\n\
             local encoded = Text.encodeUtf8(text)\n\
             if (Text.decodeUtf8(Bytes.view(encoded)) ?? \"\") ~= text then\n\
                 return 1\n\
             end\n\
             local selected = Text.encodeUtf8(Text.slice(text, 2, 2))\n\
             if (Text.decodeUtf8(Bytes.view(selected)) ?? \"\") ~= \"é中\" then\n\
                 return 2\n\
             end\n\
             local empty = Text.encodeUtf8(\"\")\n\
             if (Text.decodeUtf8(Bytes.view(empty)) ?? \"missing\") ~= \"\" then\n\
                 return 3\n\
             end\n\
             local buffer = Bytes.create()\n\
             Bytes.write(buffer, 195)\n\
             Bytes.write(buffer, 169)\n\
             local decoded = Text.decodeUtf8(buffer)\n\
             if (decoded ?? \"\") ~= \"é\" or Bytes.length(buffer) ~= 2 then\n\
                 return 4\n\
             end\n\
             Bytes.write(buffer, 255)\n\
             if Text.decodeUtf8(buffer) ~= nil or Bytes.length(buffer) ~= 3 then\n\
                 return 5\n\
             end\n\
             return 42\n\
         end\n",
    );
    let entry = mir.functions().last().expect("UTF-8 function").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified UTF-8 MIR");

    assert_eq!(
        interpreter.call(entry, &[]).expect("UTF-8 execution"),
        vec![int(42)]
    );
}

#[test]
fn portable_hexadecimal_codec_is_canonical_and_checked() {
    let (mir, types) = executable_modules(&[
        (
            "src/bytes.pop",
            include_str!("../../../../libraries/standard/pop/src/bytes.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Bytes\n\
             public function verify(): Int\n\
                 local buffer = Bytes.create()\n\
                 Bytes.write(buffer, 0)\n\
                 Bytes.write(buffer, 1)\n\
                 Bytes.write(buffer, 10)\n\
                 Bytes.write(buffer, 15)\n\
                 Bytes.write(buffer, 16)\n\
                 Bytes.write(buffer, 171)\n\
                 Bytes.write(buffer, 255)\n\
                 local source = Bytes.toBytes(buffer)\n\
                 local sourceView = Bytes.view(source)\n\
                 local encoded = hexEncode(sourceView)\n\
                 if encoded ~= \"00010a0f10abff\" then\n\
                     return 1\n\
                 end\n\
                 local decodedOptional = hexDecode(\"00010A0f10aBfF\")\n\
                 if local decoded = decodedOptional then\n\
                     local decodedView = Bytes.view(decoded)\n\
                     if not equals(sourceView, decodedView) then\n\
                         return 2\n\
                     end\n\
                 else\n\
                     return 2\n\
                 end\n\
                 local emptyOptional = hexDecode(\"\")\n\
                 if local empty = emptyOptional then\n\
                     local emptyView = Bytes.view(empty)\n\
                     if Bytes.length(emptyView) ~= 0 then\n\
                         return 3\n\
                     end\n\
                 else\n\
                     return 3\n\
                 end\n\
                 if hexDecode(\"0\") ~= nil or hexDecode(\"0x00\") ~= nil or hexDecode(\"00 01\") ~= nil or hexDecode(\"gg\") ~= nil or hexDecode(\"é0\") ~= nil then\n\
                     return 4\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("hex consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified hexadecimal MIR");

    assert_eq!(
        interpreter
            .call(entry, &[])
            .expect("portable hexadecimal execution"),
        vec![int(42)]
    );
}

#[test]
fn portable_base64_codec_matches_canonical_vectors_and_rejects_malformed_text() {
    let (mir, types) = executable_modules(&[
        (
            "src/bytes.pop",
            include_str!("../../../../libraries/standard/pop/src/bytes.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Bytes\n\
             public function verify(): Int\n\
                 local bytes = Text.encodeUtf8(\"foobar\")\n\
                 local view = Bytes.view(bytes)\n\
                 if base64Encode(view) ~= \"Zm9vYmFy\" then\n\
                     return 1\n\
                 end\n\
                 local decodedOptional = base64Decode(\"Zm9vYmFy\")\n\
                 if local decoded = decodedOptional then\n\
                     if (Text.decodeUtf8(Bytes.view(decoded)) ?? \"\") ~= \"foobar\" then\n\
                         return 2\n\
                     end\n\
                 else\n\
                     return 2\n\
                 end\n\
                 if base64Encode(Bytes.view(Text.encodeUtf8(\"f\"))) ~= \"Zg==\" or base64Encode(Bytes.view(Text.encodeUtf8(\"fo\"))) ~= \"Zm8=\" then\n\
                     return 3\n\
                 end\n\
                 if base64Encode(Bytes.view(Text.encodeUtf8(\"foo\"))) ~= \"Zm9v\" or base64Encode(Bytes.view(Text.encodeUtf8(\"foob\"))) ~= \"Zm9vYg==\" or base64Encode(Bytes.view(Text.encodeUtf8(\"fooba\"))) ~= \"Zm9vYmE=\" then\n\
                     return 5\n\
                 end\n\
                 local binary = Bytes.create()\n\
                 Bytes.write(binary, 0)\n\
                 Bytes.write(binary, 16)\n\
                 Bytes.write(binary, 131)\n\
                 Bytes.write(binary, 255)\n\
                 if base64Encode(Bytes.view(Bytes.toBytes(binary))) ~= \"ABCD/w==\" then\n\
                     return 6\n\
                 end\n\
                 local boundaries = base64Decode(\"+///\")\n\
                 if local boundaryBytes = boundaries then\n\
                     if (Bytes.get(Bytes.view(boundaryBytes), 1) ?? 0) ~= 251 or (Bytes.get(Bytes.view(boundaryBytes), 2) ?? 0) ~= 255 or (Bytes.get(Bytes.view(boundaryBytes), 3) ?? 0) ~= 255 then\n\
                         return 7\n\
                     end\n\
                 else\n\
                     return 7\n\
                 end\n\
                 if base64Decode(\"Zg=\") ~= nil or base64Decode(\"Zg\") ~= nil or base64Decode(\"====\") ~= nil or base64Decode(\"Z=== \") ~= nil then\n\
                     return 4\n\
                 end\n\
                 if base64Decode(\"Zg=A\") ~= nil or base64Decode(\"Zg==A\") ~= nil or base64Decode(\"Zh==\") ~= nil or base64Decode(\"Zm9=\") ~= nil then\n\
                     return 8\n\
                 end\n\
                 if base64Decode(\"Zm 8=\") ~= nil or base64Decode(\"Zm\\n8=\") ~= nil or base64Decode(\"Zm-8\") ~= nil or base64Decode(\"Zm_8\") ~= nil then\n\
                     return 9\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("base64 consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified base64 MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("base64 execution"),
        vec![int(42)]
    );
}

#[test]
fn portable_base32_codec_matches_canonical_vectors_and_rejects_malformed_text() {
    let (mir, types) = executable_modules(&[
        (
            "src/bytes.pop",
            include_str!("../../../../libraries/standard/pop/src/bytes.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Bytes\n\
             public function verify(): Int\n\
                 if base32Encode(Bytes.view(Text.encodeUtf8(\"\"))) ~= \"\" or base32Encode(Bytes.view(Text.encodeUtf8(\"f\"))) ~= \"MY======\" then\n\
                     return 1\n\
                 end\n\
                 if base32Encode(Bytes.view(Text.encodeUtf8(\"fo\"))) ~= \"MZXQ====\" or base32Encode(Bytes.view(Text.encodeUtf8(\"foo\"))) ~= \"MZXW6===\" then\n\
                     return 2\n\
                 end\n\
                 if base32Encode(Bytes.view(Text.encodeUtf8(\"foob\"))) ~= \"MZXW6YQ=\" or base32Encode(Bytes.view(Text.encodeUtf8(\"fooba\"))) ~= \"MZXW6YTB\" or base32Encode(Bytes.view(Text.encodeUtf8(\"foobar\"))) ~= \"MZXW6YTBOI======\" then\n\
                     return 3\n\
                 end\n\
                 local binary = Bytes.create()\n\
                 Bytes.write(binary, 0)\n\
                 Bytes.write(binary, 16)\n\
                 Bytes.write(binary, 131)\n\
                 Bytes.write(binary, 255)\n\
                 if base32Encode(Bytes.view(Bytes.toBytes(binary))) ~= \"AAIIH7Y=\" then\n\
                     return 4\n\
                 end\n\
                 local decodedOptional = base32Decode(\"HY7UAQK2MF5A====\")\n\
                 if local decoded = decodedOptional then\n\
                     if Bytes.length(Bytes.view(decoded)) ~= 7 or (Bytes.get(Bytes.view(decoded), 1) ?? 0) ~= 62 or (Bytes.get(Bytes.view(decoded), 7) ?? 0) ~= 122 then\n\
                         return 5\n\
                     end\n\
                 else\n\
                     return 5\n\
                 end\n\
                 if base32Decode(\"MY=====\") ~= nil or base32Decode(\"MY\") ~= nil or base32Decode(\"========\") ~= nil or base32Decode(\"MY=====A\") ~= nil then\n\
                     return 6\n\
                 end\n\
                 if base32Decode(\"my======\") ~= nil or base32Decode(\"M0======\") ~= nil or base32Decode(\"M1======\") ~= nil or base32Decode(\"M Y=====\") ~= nil then\n\
                     return 7\n\
                 end\n\
                 if base32Decode(\"MZ======\") ~= nil or base32Decode(\"MZXR====\") ~= nil or base32Decode(\"MZXW7===\") ~= nil or base32Decode(\"MZXW6YR=\") ~= nil then\n\
                     return 8\n\
                 end\n\
                 if base32Decode(\"MZXW6Y==\") ~= nil or base32Decode(\"MZXW6YQ=A\") ~= nil then\n\
                     return 9\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("base32 consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified base32 MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("base32 execution"),
        vec![int(42)]
    );
}

#[test]
fn portable_bytes_bitwise_transforms_cover_complete_bytes_and_checked_lengths() {
    let (mir, types) = executable_modules(&[
        (
            "src/bytes.pop",
            include_str!("../../../../libraries/standard/pop/src/bytes.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Bytes\n\
             public function verify(): Int\n\
                 local leftBuffer = Bytes.create()\n\
                 Bytes.write(leftBuffer, 0)\n\
                 Bytes.write(leftBuffer, 170)\n\
                 Bytes.write(leftBuffer, 240)\n\
                 local rightBuffer = Bytes.create()\n\
                 Bytes.write(rightBuffer, 255)\n\
                 Bytes.write(rightBuffer, 204)\n\
                 Bytes.write(rightBuffer, 15)\n\
                 local leftBytes = Bytes.toBytes(leftBuffer)\n\
                 local rightBytes = Bytes.toBytes(rightBuffer)\n\
                 local left = Bytes.view(leftBytes)\n\
                 local right = Bytes.view(rightBytes)\n\
                 if local value = bitwiseAnd(left, right) then\n\
                     local view = Bytes.view(value)\n\
                     if (Bytes.get(view, 1) ?? 1) ~= 0 or (Bytes.get(view, 2) ?? 0) ~= 136 or (Bytes.get(view, 3) ?? 1) ~= 0 then\n\
                         return 1\n\
                     end\n\
                 else\n\
                     return 1\n\
                 end\n\
                 if local value = bitwiseOr(left, right) then\n\
                     local view = Bytes.view(value)\n\
                     if (Bytes.get(view, 1) ?? 0) ~= 255 or (Bytes.get(view, 2) ?? 0) ~= 238 or (Bytes.get(view, 3) ?? 0) ~= 255 then\n\
                         return 2\n\
                     end\n\
                 else\n\
                     return 2\n\
                 end\n\
                 if local value = bitwiseXor(left, right) then\n\
                     local view = Bytes.view(value)\n\
                     if (Bytes.get(view, 1) ?? 0) ~= 255 or (Bytes.get(view, 2) ?? 0) ~= 102 or (Bytes.get(view, 3) ?? 0) ~= 255 then\n\
                         return 3\n\
                     end\n\
                 else\n\
                     return 3\n\
                 end\n\
                 local inverted = bitwiseNot(left)\n\
                 local invertedView = Bytes.view(inverted)\n\
                 if (Bytes.get(invertedView, 1) ?? 0) ~= 255 or (Bytes.get(invertedView, 2) ?? 0) ~= 85 or (Bytes.get(invertedView, 3) ?? 0) ~= 15 then\n\
                     return 4\n\
                 end\n\
                 local shortBytes = Text.encodeUtf8(\"x\")\n\
                 if bitwiseAnd(left, Bytes.view(shortBytes)) ~= nil or bitwiseOr(left, Bytes.view(shortBytes)) ~= nil or bitwiseXor(left, Bytes.view(shortBytes)) ~= nil then\n\
                     return 5\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("bitwise consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified bitwise MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("bitwise execution"),
        vec![int(42)]
    );
}

#[test]
fn portable_bytes_bitwise_transforms_preserve_empty_length() {
    let (mir, types) = executable_modules(&[
        (
            "src/bytes.pop",
            include_str!("../../../../libraries/standard/pop/src/bytes.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Bytes\n\
             public function verify(): Int\n\
                 local firstBytes = Text.encodeUtf8(\"\")\n\
                 local secondBytes = Text.encodeUtf8(\"\")\n\
                 local first = Bytes.view(firstBytes)\n\
                 local second = Bytes.view(secondBytes)\n\
                 local inverted = bitwiseNot(first)\n\
                 local invertedView = Bytes.view(inverted)\n\
                 if Bytes.length(invertedView) ~= 0 then\n\
                     return 1\n\
                 end\n\
                 if local combined = bitwiseXor(first, second) then\n\
                     local combinedView = Bytes.view(combined)\n\
                     if Bytes.length(combinedView) ~= 0 then\n\
                         return 2\n\
                     end\n\
                 else\n\
                     return 2\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .last()
        .expect("empty bitwise consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified empty bitwise MIR");
    assert_eq!(
        interpreter
            .call(entry, &[])
            .expect("empty bitwise execution"),
        vec![int(42)]
    );
}

#[test]
fn essential_text_algorithms_are_unicode_safe_linear_and_checked() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Text\n\
             public function verify(): Int\n\
                 if trim(\"\t\u{a0} hello \u{3000}\\n\") ~= \"hello\" then\n\
                     return 1\n\
                 end\n\
                 if trimStart(\"\u{2003}\u{2003}é \") ~= \"é \" or trimEnd(\" é\u{202f}\") ~= \" é\" then\n\
                     return 2\n\
                 end\n\
                 if trim(\"\u{1680}\u{205f}\") ~= \"\" or trim(\"中\") ~= \"中\" then\n\
                     return 3\n\
                 end\n\
                 if replace(\"aé中éz\", \"é\", \"--\") ~= \"a--中--z\" or replace(\"aaaa\", \"aa\", \"b\") ~= \"bb\" then\n\
                     return 4\n\
                 end\n\
                 if replace(\"same\", \"\", \"x\") ~= \"same\" or replace(\"same\", \"z\", \"x\") ~= \"same\" then\n\
                     return 5\n\
                 end\n\
                 local pieces = split(\"éaé中é\", \"é\")\n\
                 if List.length(pieces) ~= 4 or List.get(pieces, 1) ~= \"\" or List.get(pieces, 2) ~= \"a\" or List.get(pieces, 3) ~= \"中\" or List.get(pieces, 4) ~= \"\" then\n\
                     return 6\n\
                 end\n\
                 local unsplit = split(\"abc\", \"\")\n\
                 if List.length(unsplit) ~= 1 or List.get(unsplit, 1) ~= \"abc\" then\n\
                     return 7\n\
                 end\n\
                 local values = List.create<<String>>()\n\
                 List.add(values, \"a\")\n\
                 List.add(values, \"中\")\n\
                 List.add(values, \"\")\n\
                 if join(values, \"·\") ~= \"a·中·\" then\n\
                     return 8\n\
                 end\n\
                 local empty = List.create<<String>>()\n\
                 if join(empty, \",\") ~= \"\" then\n\
                     return 9\n\
                 end\n\
                 if (parseInt(\"0\") ?? 99) ~= 0 or (parseInt(\"+42\") ?? 0) ~= 42 or (parseInt(\"-42\") ?? 0) ~= -42 then\n\
                     return 10\n\
                 end\n\
                 if (parseInt(\"9223372036854775807\") ?? 0) ~= 9223372036854775807 or (parseInt(\"-9223372036854775808\") ?? 0) ~= -9223372036854775807 - 1 then\n\
                     return 11\n\
                 end\n\
                 if parseInt(\"9223372036854775808\") ~= nil or parseInt(\"-9223372036854775809\") ~= nil then\n\
                     return 12\n\
                 end\n\
                 if parseInt(\"\") ~= nil or parseInt(\"+\") ~= nil or parseInt(\" 1\") ~= nil or parseInt(\"１２\") ~= nil or parseInt(\"1x\") ~= nil then\n\
                     return 13\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("Text consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Text MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Text execution"),
        vec![int(42)]
    );
}

#[test]
fn essential_text_search_returns_exact_scalar_boundaries() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Text\n\
             public function verify(): Int\n\
                 if not startsWith(\"é中😀z\", \"é中\") or startsWith(\"é中\", \"é中😀\") or not startsWith(\"x\", \"\") then\n\
                     return 1\n\
                 end\n\
                 if not endsWith(\"é中😀z\", \"😀z\") or endsWith(\"é中\", \"xé中\") or not endsWith(\"\", \"\") then\n\
                     return 2\n\
                 end\n\
                 if not contains(\"aé中😀é\", \"中😀\") or contains(\"aé中😀é\", \"É\") or not contains(\"\", \"\") then\n\
                     return 3\n\
                 end\n\
                 if (indexOf(\"aé中😀é\", \"é\", 1) ?? 0) ~= 2 or (indexOf(\"aé中😀é\", \"é\", 3) ?? 0) ~= 5 then\n\
                     return 4\n\
                 end\n\
                 if (indexOf(\"aé中😀é\", \"😀\", 1) ?? 0) ~= 4 or (indexOf(\"aaaa\", \"aa\", 2) ?? 0) ~= 2 then\n\
                     return 5\n\
                 end\n\
                 if (indexOf(\"aé中😀é\", \"\", 6) ?? 0) ~= 6 or indexOf(\"aé中😀é\", \"\", 0) ~= nil or indexOf(\"aé中😀é\", \"\", 7) ~= nil then\n\
                     return 6\n\
                 end\n\
                 if indexOf(\"aé中😀é\", \"missing\", 1) ~= nil then\n\
                     return 7\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("Text search consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Text search MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Text search execution"),
        vec![int(42)]
    );
}

#[test]
fn essential_text_ascii_casing_preserves_non_ascii_bytes() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Text\n\
             public function verify(): Int\n\
                 if toAsciiLower(\"HTTP-É中😀-42\") ~= \"http-É中😀-42\" then\n\
                     return 1\n\
                 end\n\
                 if toAsciiUpper(\"http-é中😀-42\") ~= \"HTTP-é中😀-42\" then\n\
                     return 2\n\
                 end\n\
                 if toAsciiLower(\"\") ~= \"\" or toAsciiUpper(\"Already\") ~= \"ALREADY\" then\n\
                     return 3\n\
                 end\n\
                 if not equalsAsciiIgnoreCase(\"Content-TYPE\", \"content-type\") then\n\
                     return 4\n\
                 end\n\
                 if equalsAsciiIgnoreCase(\"É\", \"é\") or equalsAsciiIgnoreCase(\"abc\", \"ab\") or equalsAsciiIgnoreCase(\"abc\", \"abd\") then\n\
                     return 5\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("ASCII casing consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified ASCII casing MIR");
    assert_eq!(
        interpreter
            .call(entry, &[])
            .expect("ASCII casing execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_semantic_versions_parse_order_format_and_match() {
    let (mir, types) = executable_modules(&[
        (
            "src/math.pop",
            include_str!("../../../../libraries/standard/pop/src/math.pop"),
        ),
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/version.pop",
            include_str!("../../../../libraries/standard/pop/src/version.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Version\n\
             private function required(text: String): Value\n\
                 local fallback: Value = { major = 0, minor = 0, patch = 0, prerelease = \"\", build = \"\" }\n\
                 return parse(text) ?? fallback\n\
             end\n\
             public function verify(): Int\n\
                 local complete = required(\"1.2.3-alpha.1+linux\")\n\
                 if format(complete) ~= \"1.2.3-alpha.1+linux\" then\n\
                     return 1\n\
                 end\n\
                 if parse(\"01.2.3\") ~= nil or parse(\"1.2.3-\") ~= nil or parse(\"1.2.3-alpha..1\") ~= nil or parse(\"1.2.3-α\") ~= nil or parse(\"2147483647.0.0\") ~= nil then\n\
                     return 2\n\
                 end\n\
                 if compare(required(\"1.0.0-alpha\"), required(\"1.0.0-alpha.1\")) >= 0 or compare(required(\"1.0.0-alpha.1\"), required(\"1.0.0-alpha.beta\")) >= 0 then\n\
                     return 3\n\
                 end\n\
                 if compare(required(\"1.0.0-alpha.beta\"), required(\"1.0.0-beta\")) >= 0 or compare(required(\"1.0.0-beta\"), required(\"1.0.0-beta.2\")) >= 0 then\n\
                     return 4\n\
                 end\n\
                 if compare(required(\"1.0.0-beta.2\"), required(\"1.0.0-beta.11\")) >= 0 or compare(required(\"1.0.0-beta.11\"), required(\"1.0.0-rc.1\")) >= 0 or compare(required(\"1.0.0-rc.1\"), required(\"1.0.0\")) >= 0 then\n\
                     return 5\n\
                 end\n\
                 if compare(required(\"1.2.3+one\"), required(\"1.2.3+two\")) ~= 0 then\n\
                     return 6\n\
                 end\n\
                 if not matches(required(\"1.4.5\"), \"^1.2.3\") or matches(required(\"2.0.0\"), \"^1.2.3\") then\n\
                     return 7\n\
                 end\n\
                 if not matches(required(\"1.2.9\"), \"~1.2.3\") or matches(required(\"1.3.0\"), \"~1.2.3\") then\n\
                     return 8\n\
                 end\n\
                 if not matches(required(\"1.2.3+build\"), \"=1.2.3\") or not matches(required(\"1.2.4\"), \">1.2.3\") or matches(required(\"1.2.3\"), \">=broken\") then\n\
                     return 9\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("Version consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Version MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Version execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_media_types_parse_format_lookup_and_match() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/mime.pop",
            include_str!("../../../../libraries/standard/pop/src/mime.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Mime\n\
             private function fallback(): Value\n\
                 local parameters = List.create<<Parameter>>()\n\
                 return { mediaType = \"application\", subtype = \"octet-stream\", parameters = parameters }\n\
             end\n\
             private function required(text: String): Value\n\
                 return parse(text) ?? fallback()\n\
             end\n\
             public function verify(): Int\n\
                 local plain = required(\"Text/Plain; Charset=\\\"utf-8\\\"; title=\\\"a b\\\"; note=\\\"a;b\\\"\")\n\
                 if plain.mediaType ~= \"text\" or plain.subtype ~= \"plain\" then\n\
                     return 1\n\
                 end\n\
                 if (parameter(plain, \"CHARSET\") ?? \"\") ~= \"utf-8\" or (parameter(plain, \"title\") ?? \"\") ~= \"a b\" or (parameter(plain, \"note\") ?? \"\") ~= \"a;b\" then\n\
                     return 2\n\
                 end\n\
                 if format(plain) ~= \"text/plain; charset=utf-8; title=\\\"a b\\\"; note=\\\"a;b\\\"\" then\n\
                     return 3\n\
                 end\n\
                 local escaped = required(\"text/plain; title=\\\"a\\\\\\\"b\\\"\")\n\
                 if (parameter(escaped, \"title\") ?? \"\") ~= \"a\\\"b\" or format(escaped) ~= \"text/plain; title=\\\"a\\\\\\\"b\\\"\" then\n\
                     return 4\n\
                 end\n\
                 if parse(\"text\") ~= nil or parse(\"text/plain;\") ~= nil or parse(\"text/plain; A=1; a=2\") ~= nil or parse(\"téxt/plain\") ~= nil or parse(\"text/plain; x=\\\"broken\") ~= nil then\n\
                     return 5\n\
                 end\n\
                 if not matches(plain, \"text/plain\") or not matches(plain, \"TEXT/*\") or not matches(plain, \"*/*\") then\n\
                     return 6\n\
                 end\n\
                 if matches(plain, \"application/*\") or matches(plain, \"*/plain\") or matches(plain, \"text/plain; charset=utf-8\") then\n\
                     return 7\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Mime consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Mime MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Mime execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_uri_references_parse_code_and_resolve() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/uri.pop",
            include_str!("../../../../libraries/standard/pop/src/uri.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Uri\n\
             private function resolved(base: Value, referenceText: String): String\n\
                 local reference = parse(referenceText) ?? base\n\
                 return format(resolve(base, reference))\n\
             end\n\
             public function verify(): Int\n\
                 if local absolute = parse(\"HTTPS://example.test/a%20b?x=1#part\") then\n\
                     if absolute.scheme ~= \"https\" or (absolute.authority ?? \"\") ~= \"example.test\" or absolute.path ~= \"/a%20b\" or (absolute.query ?? \"\") ~= \"x=1\" or (absolute.fragment ?? \"\") ~= \"part\" then\n\
                         return 1\n\
                     end\n\
                     if format(absolute) ~= \"https://example.test/a%20b?x=1#part\" then\n\
                         return 2\n\
                     end\n\
                 else\n\
                     return 1\n\
                 end\n\
                 if local empty = parse(\"https://example.test?#\") then\n\
                     if empty.query == nil or empty.fragment == nil or format(empty) ~= \"https://example.test?#\" then\n\
                         return 3\n\
                     end\n\
                 else\n\
                     return 3\n\
                 end\n\
                 if parse(\"1http:x\") ~= nil or parse(\"a b\") ~= nil or parse(\"a%2\") ~= nil or parse(\"é\") ~= nil or parse(\"a#b#c\") ~= nil then\n\
                     return 4\n\
                 end\n\
                 if local relative = parse(\"a/b:c\") then\n\
                     if relative.scheme ~= \"\" or relative.path ~= \"a/b:c\" then\n\
                         return 5\n\
                     end\n\
                 else\n\
                     return 5\n\
                 end\n\
                 if local fragmentColon = parse(\"abc#d:e\") then\n\
                     if fragmentColon.scheme ~= \"\" or fragmentColon.path ~= \"abc\" or (fragmentColon.fragment ?? \"\") ~= \"d:e\" then\n\
                         return 5\n\
                     end\n\
                 else\n\
                     return 5\n\
                 end\n\
                 if (percentEncode(\"é 中\") ?? \"\") ~= \"%C3%A9%20%E4%B8%AD\" or (percentDecode(\"%C3%A9%20%E4%B8%AD\") ?? \"\") ~= \"é 中\" then\n\
                     return 6\n\
                 end\n\
                 if percentDecode(\"%\") ~= nil or percentDecode(\"%GG\") ~= nil or percentDecode(\"%FF\") ~= nil then\n\
                     return 7\n\
                 end\n\
                 if local base = parse(\"http://a/b/c/d;p?q\") then\n\
                     if resolved(base, \"g:h\") ~= \"g:h\" or resolved(base, \"g\") ~= \"http://a/b/c/g\" or resolved(base, \"./g\") ~= \"http://a/b/c/g\" then\n\
                         return 8\n\
                     end\n\
                     if resolved(base, \"/g\") ~= \"http://a/g\" or resolved(base, \"//g\") ~= \"http://g\" or resolved(base, \"?y\") ~= \"http://a/b/c/d;p?y\" then\n\
                         return 9\n\
                     end\n\
                     if resolved(base, \"g?y#s\") ~= \"http://a/b/c/g?y#s\" or resolved(base, \"#s\") ~= \"http://a/b/c/d;p?q#s\" then\n\
                         return 10\n\
                     end\n\
                     if resolved(base, \".\") ~= \"http://a/b/c/\" or resolved(base, \"..\") ~= \"http://a/b/\" or resolved(base, \"../../g\") ~= \"http://a/g\" then\n\
                         return 11\n\
                     end\n\
                 else\n\
                     return 8\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Uri consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Uri MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Uri execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_guid_values_round_trip_and_inject_version_four_bytes() {
    let (mir, types) = executable_modules(&[
        (
            "src/guid.pop",
            include_str!("../../../../libraries/standard/pop/src/guid.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Guid\n\
             public function verify(): Int\n\
                 if local parsed = parse(\"00112233-4455-1677-8899-aabbccddeeff\") then\n\
                     if format(parsed) ~= \"00112233-4455-1677-8899-aabbccddeeff\" or isVersion4(parsed) then\n\
                         return 1\n\
                     end\n\
                     local bytes = toBytes(parsed)\n\
                     if local roundTrip = fromBytes(bytes) then\n\
                         if format(roundTrip) ~= format(parsed) then\n\
                             return 3\n\
                         end\n\
                     else\n\
                         return 3\n\
                     end\n\
                 else\n\
                     return 1\n\
                 end\n\
                 if local uppercase = parse(\"00112233-4455-4677-8899-AABBCCDDEEFF\") then\n\
                     if format(uppercase) ~= \"00112233-4455-4677-8899-aabbccddeeff\" then\n\
                         return 4\n\
                     end\n\
                 else\n\
                     return 4\n\
                 end\n\
                 if parse(\"\") ~= nil or parse(\"{00112233-4455-4677-8899-aabbccddeeff}\") ~= nil or parse(\"00112233445546778899aabbccddeeff\") ~= nil or parse(\"00112233-4455-4677-8899-aabbccddeefg\") ~= nil then\n\
                     return 5\n\
                 end\n\
                 local empty = Bytes.toBytes(Bytes.create())\n\
                 if fromBytes(empty) ~= nil then\n\
                     return 6\n\
                 end\n\
                 local randomBuffer = Bytes.withCapacity(16)\n\
                 for index = 0, 15 do\n\
                     Bytes.write(randomBuffer, Byte(index))\n\
                 end\n\
                 local randomBytes = Bytes.toBytes(randomBuffer)\n\
                 if local generated = newVersion4(randomBytes) then\n\
                     if format(generated) ~= \"00010203-0405-4607-8809-0a0b0c0d0e0f\" or not isVersion4(generated) then\n\
                         return 7\n\
                     end\n\
                 else\n\
                     return 7\n\
                 end\n\
                 if newVersion4(empty) ~= nil then\n\
                     return 8\n\
                 end\n\
                 local nilValue: Value = { firstWord = UInt32(0), secondWord = UInt32(0), thirdWord = UInt32(0), fourthWord = UInt32(0) }\n\
                 local unknownValue: Value = { firstWord = UInt32(1), secondWord = UInt32(0), thirdWord = UInt32(0), fourthWord = UInt32(0) }\n\
                 if not isNil(nilValue) or isNil(unknownValue) or isVersion4(unknownValue) then\n\
                     return 9\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Guid consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Guid MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Guid execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_portable_paths_normalize_and_inspect_lexically() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/path.pop",
            include_str!("../../../../libraries/standard/pop/src/path.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Path\n\
             private function required(text: String): Value\n\
                 return normalize(text) ?? { text = \"invalid\", absolute = false }\n\
             end\n\
             public function verify(): Int\n\
                 if format(required(\"\")) ~= \".\" or format(required(\"/a//b/../c/.\")) ~= \"/a/c\" then\n\
                     return 1\n\
                 end\n\
                 if format(required(\"../../a/../b\")) ~= \"../../b\" or format(required(\"/../../a\")) ~= \"/a\" then\n\
                     return 2\n\
                 end\n\
                 if normalize(\"a\\\\b\") ~= nil then\n\
                     return 3\n\
                 end\n\
                 local base = required(\"/a/b\")\n\
                 if not isAbsolute(base) or format(join(base, \"../c\") ?? required(\".\")) ~= \"/a/c\" or join(base, \"/c\") ~= nil then\n\
                     return 4\n\
                 end\n\
                 local file = required(\"/a/archive.tar.gz\")\n\
                 if format(parent(file) ?? required(\".\")) ~= \"/a\" or (name(file) ?? \"\") ~= \"archive.tar.gz\" or (extension(file) ?? \"\") ~= \"gz\" then\n\
                     return 5\n\
                 end\n\
                 if extension(required(\".env\")) ~= nil or extension(required(\"name.\")) ~= nil then\n\
                     return 6\n\
                 end\n\
                 if parent(required(\"/\")) ~= nil or name(required(\".\")) ~= nil then\n\
                     return 7\n\
                 end\n\
                 if format(required(\"dados/ação.txt\")) ~= \"dados/ação.txt\" then\n\
                     return 8\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Path consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Path MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Path execution"),
        vec![int(42)]
    );
}

#[test]
fn canonical_durations_preserve_exact_signed_units() {
    let (mir, types) = executable_modules(&[
        (
            "src/time.pop",
            include_str!("../../../../libraries/standard/pop/src/time.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Time\n\
             public function verify(): Int\n\
                 local positive = fromMilliseconds(1500)\n\
                 if secondsPart(positive) ~= 1 or nanosecondsPart(positive) ~= 500000000 then\n\
                     return 1\n\
                 end\n\
                 local negative = fromMilliseconds(-1)\n\
                 if secondsPart(negative) ~= -1 or nanosecondsPart(negative) ~= 999000000 or not isNegative(negative) then\n\
                     return 2\n\
                 end\n\
                 local finalNano = fromNanoseconds(-1)\n\
                 if secondsPart(finalNano) ~= -1 or nanosecondsPart(finalNano) ~= 999999999 then\n\
                     return 3\n\
                 end\n\
                 if not isZero(fromSeconds(0)) or compare(negative, positive) ~= -1 or compare(positive, negative) ~= 1 or compare(positive, fromNanoseconds(1500000000)) ~= 0 then\n\
                     return 4\n\
                 end\n\
                 local low = fromSeconds(-9000000000000000000)\n\
                 local high = fromSeconds(9000000000000000000)\n\
                 if compare(low, high) ~= -1 then\n\
                     return 5\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Time consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Time MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Time execution"),
        vec![int(42)]
    );
}

#[test]
fn deterministic_test_clocks_advance_and_expire_exactly() {
    let (mir, types) = executable_modules(&[
        (
            "src/time.pop",
            include_str!("../../../../libraries/standard/pop/src/time.pop"),
        ),
        (
            "src/timeClock.pop",
            include_str!("../../../../libraries/standard/pop/src/timeClock.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Time\n\
             public function verify(): Int?\n\
                 local start = instant(10, 900000000)?\n\
                 local clock = testClock(start)?\n\
                 local before = now(clock)\n\
                 if before.seconds ~= 10 or before.nanoseconds ~= 900000000 then\n\
                     return 1\n\
                 end\n\
                 if not advance(clock, fromMilliseconds(200)) then\n\
                     return 2\n\
                 end\n\
                 local after = now(clock)\n\
                 if after.seconds ~= 11 or after.nanoseconds ~= 100000000 then\n\
                     return 3\n\
                 end\n\
                 local deadline = deadlineAfter(clock, fromMilliseconds(500))?\n\
                 if isExpired(clock, deadline) then\n\
                     return 4\n\
                 end\n\
                 if not advance(clock, fromMilliseconds(500)) or not isExpired(clock, deadline) then\n\
                     return 5\n\
                 end\n\
                 if advance(clock, fromNanoseconds(-1)) then\n\
                     return 6\n\
                 end\n\
                 local unchanged = now(clock)\n\
                 if unchanged.seconds ~= 11 or unchanged.nanoseconds ~= 600000000 then\n\
                     return 7\n\
                 end\n\
                 local nearEnd = instant(2147483646, 999999999)?\n\
                 local finalClock = testClock(nearEnd)?\n\
                 if advance(finalClock, fromNanoseconds(1)) or deadlineAfter(finalClock, fromNanoseconds(1)) ~= nil then\n\
                     return 8\n\
                 end\n\
                 if instant(-1, 0) ~= nil or instant(0, -1) ~= nil or instant(0, 1000000000) ~= nil or instant(2147483647, 0) ~= nil then\n\
                     return 9\n\
                 end\n\
                 local invalid: Instant = { seconds = -1, nanoseconds = 0 }\n\
                 if testClock(invalid) ~= nil then\n\
                     return 10\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("TestClock consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified TestClock MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("TestClock execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_gregorian_dates_validate_and_compare_exactly() {
    let (mir, types) = executable_modules(&[
        (
            "src/timeDate.pop",
            include_str!("../../../../libraries/standard/pop/src/timeDate.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Time\n\
             public function verify(): Int?\n\
                 local leap = date(2024, 2, 29)?\n\
                 local next = date(2024, 3, 1)?\n\
                 if (daysInMonth(2024, 2) ?? 0) ~= 29 or (daysInMonth(2023, 2) ?? 0) ~= 28 then\n\
                     return 1\n\
                 end\n\
                 if not isLeapYear(2024) or isLeapYear(1900) or not isLeapYear(2000) then\n\
                     return 2\n\
                 end\n\
                 if compareDates(leap, next) ~= -1 or compareDates(next, leap) ~= 1 or compareDates(leap, leap) ~= 0 then\n\
                     return 3\n\
                 end\n\
                 if date(0, 1, 1) ~= nil or date(10000, 1, 1) ~= nil or date(2023, 2, 29) ~= nil or date(2024, 13, 1) ~= nil then\n\
                     return 4\n\
                 end\n\
                 if daysInMonth(0, 1) ~= nil or daysInMonth(2024, 0) ~= nil or daysInMonth(2024, 13) ~= nil then\n\
                     return 5\n\
                 end\n\
                 local first = date(1, 1, 1)?\n\
                 local final = date(9999, 12, 31)?\n\
                 if compareDates(first, final) ~= -1 then\n\
                     return 6\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Date consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Date MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Date execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_civil_time_values_keep_local_and_offset_meanings_distinct() {
    let (mir, types) = executable_modules(&[
        (
            "src/timeDate.pop",
            include_str!("../../../../libraries/standard/pop/src/timeDate.pop"),
        ),
        (
            "src/timeDateTime.pop",
            include_str!("../../../../libraries/standard/pop/src/timeDateTime.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Time\n\
             public function verify(): Int?\n\
                 local day = date(2024, 2, 29)?\n\
                 local time = timeOfDay(23, 59, 59, 999999999)?\n\
                 local localValue = localDateTime(day, time)?\n\
                 local offset = utcOffset(-18000)?\n\
                 local complete = offsetDateTime(localValue, offset)?\n\
                 if complete.dateTime.date.day ~= 29 or complete.dateTime.time.hour ~= 23 or complete.offset.seconds ~= -18000 then\n\
                     return 1\n\
                 end\n\
                 local zero = utcOffset(0)?\n\
                 if not isUtc(zero) or isUtc(offset) then\n\
                     return 2\n\
                 end\n\
                 if timeOfDay(-1, 0, 0, 0) ~= nil or timeOfDay(24, 0, 0, 0) ~= nil or timeOfDay(0, 60, 0, 0) ~= nil or timeOfDay(0, 0, 60, 0) ~= nil or timeOfDay(0, 0, 0, 1000000000) ~= nil then\n\
                     return 3\n\
                 end\n\
                 if utcOffset(-64801) ~= nil or utcOffset(64801) ~= nil then\n\
                     return 4\n\
                 end\n\
                 local invalidTime: TimeOfDay = { hour = 24, minute = 0, second = 0, nanosecond = 0 }\n\
                 if localDateTime(day, invalidTime) ~= nil then\n\
                     return 5\n\
                 end\n\
                 local invalidOffset: UtcOffset = { seconds = 64801 }\n\
                 if offsetDateTime(localValue, invalidOffset) ~= nil then\n\
                     return 6\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .last()
        .expect("civil Time consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified civil Time MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("civil Time execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_locale_tags_canonicalize_without_ambient_discovery() {
    let (mir, types) = executable_modules(&[
        (
            "src/unicode.pop",
            include_str!("../../../../libraries/standard/pop/src/unicode.pop"),
        ),
        (
            "src/text.pop",
            include_str!("../../../../libraries/standard/pop/src/text.pop"),
        ),
        (
            "src/locale.pop",
            include_str!("../../../../libraries/standard/pop/src/locale.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Locale\n\
             public function verify(): Int?\n\
                 local portuguese = parse(\"pt-br\")?\n\
                 local traditional = parse(\"zh-hant-tw\")?\n\
                 if (format(portuguese) ?? \"\") ~= \"pt-BR\" or (format(traditional) ?? \"\") ~= \"zh-Hant-TW\" then\n\
                     return 1\n\
                 end\n\
                 local other = parse(\"pt-PT\")?\n\
                 if not sameLanguage(portuguese, other) or sameLanguage(portuguese, traditional) then\n\
                     return 2\n\
                 end\n\
                 if parse(\"e\") ~= nil or parse(\"9n\") ~= nil or parse(\"en_\") ~= nil or parse(\"en--US\") ~= nil or parse(\"en-US-extra\") ~= nil or parse(\"é\") ~= nil then\n\
                     return 3\n\
                 end\n\
                 local valid = parse(\"en\")?\n\
                 local invalid: Tag = { language = \"e\", script = valid.script, region = valid.region }\n\
                 if format(invalid) ~= nil then\n\
                     return 4\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Locale consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Locale MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Locale execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_text_globs_match_unicode_scalars_and_escapes() {
    let (mir, types) = executable_modules(&[
        (
            "src/glob.pop",
            include_str!("../../../../libraries/standard/pop/src/glob.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Glob\n\
             public function verify(): Int?\n\
                 local wildcard = compile(\"a*?c\")?\n\
                 if not matches(wildcard, \"abxc\") or matches(wildcard, \"ac\") or matches(wildcard, \"abxcd\") then\n\
                     return 1\n\
                 end\n\
                 local escaped = compile(\"a\\\\*b\")?\n\
                 if not matches(escaped, \"a*b\") or matches(escaped, \"axxb\") then\n\
                     return 2\n\
                 end\n\
                 local scalar = compile(\"?.txt\")?\n\
                 if not matches(scalar, \"😀.txt\") or matches(scalar, \"ab.txt\") then\n\
                     return 3\n\
                 end\n\
                 local empty = compile(\"\")?\n\
                 if not matches(empty, \"\") or matches(empty, \"x\") or compile(\"\\\\\") ~= nil then\n\
                     return 4\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Glob consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Glob MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Glob execution"),
        vec![int(42)]
    );
}

#[test]
fn bounded_csv_rows_parse_and_format_strict_quoting() {
    let (mir, types) = executable_modules(&[
        (
            "src/csv.pop",
            include_str!("../../../../libraries/standard/pop/src/csv.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Csv\n\
             public function verify(): Int?\n\
                 local rows = parse(\"a,\\\"b,c\\\"\\r\\n\\\"x\\\"\\\"y\\\",z\\n\")?\n\
                 if List.length(rows) ~= 2 then\n\
                     return 11\n\
                 end\n\
                 if List.get(List.get(rows, 1), 2) ~= \"b,c\" then\n\
                     return 12\n\
                 end\n\
                 if List.get(List.get(rows, 2), 1) ~= \"x\\\"y\" then\n\
                     return 13\n\
                 end\n\
                 if (format(rows) ?? \"\") ~= \"a,\\\"b,c\\\"\\r\\n\\\"x\\\"\\\"y\\\",z\" then\n\
                     return 2\n\
                 end\n\
                 local embedded = parse(\"\\\"a\\nb\\\",c\")?\n\
                 if List.get(List.get(embedded, 1), 1) ~= \"a\\nb\" then\n\
                     return 3\n\
                 end\n\
                 local empty = parse(\"\")?\n\
                 if List.length(empty) ~= 1 or List.get(List.get(empty, 1), 1) ~= \"\" then\n\
                     return 4\n\
                 end\n\
                 if parse(\"a\\rb\") ~= nil or parse(\"a\\\"b\") ~= nil or parse(\"\\\"a\\\"x\") ~= nil or parse(\"\\\"open\") ~= nil then\n\
                     return 5\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Csv consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Csv MIR");
    assert_eq!(
        interpreter.call(entry, &[]).expect("Csv execution"),
        vec![int(42)]
    );
}

#[test]
fn materializing_sequence_order_and_equality_are_stable_and_short_circuit() {
    let (mir, types) = executable_modules(&[
        (
            "src/sequence.pop",
            include_str!("../../../../libraries/standard/pop/src/sequence.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Sequence\n\
             private record Candidate\n\
                 id: Int\n\
                 key: Int\n\
             end\n\
             public function verify(): Int\n\
                 local first: Candidate = { id = 1, key = 2 }\n\
                 local second: Candidate = { id = 2, key = 1 }\n\
                 local third: Candidate = { id = 3, key = 2 }\n\
                 local fourth: Candidate = { id = 4, key = 1 }\n\
                 local values: {Candidate} = {first, second, third, fourth}\n\
                 local ordered = sortBy<<Candidate, {Candidate}>>(values, function(value: Candidate): Int\n\
                     return value.key\n\
                 end)\n\
                 local orderedFirst = List.get(ordered, 1)\n\
                 local orderedSecond = List.get(ordered, 2)\n\
                 local orderedThird = List.get(ordered, 3)\n\
                 local orderedFourth = List.get(ordered, 4)\n\
                 if orderedFirst.id ~= 2 or orderedSecond.id ~= 4 or orderedThird.id ~= 1 or orderedFourth.id ~= 3 then\n\
                     return 1\n\
                 end\n\
                 local sourceFirst = Array.get(values, 1)\n\
                 if sourceFirst.id ~= 1 then\n\
                     return 2\n\
                 end\n\
                 local numbers: {Int} = {1, 2, 3}\n\
                 local descending: {Int} = {3, 2, 1}\n\
                 local orderedNumbers = sort<<Int, {Int}>>(descending, function(left: Int, right: Int): Int\n\
                     if left < right then\n\
                         return -1\n\
                     end\n\
                     if left > right then\n\
                         return 1\n\
                     end\n\
                     return 0\n\
                 end)\n\
                 if List.get(orderedNumbers, 1) ~= 1 or List.get(orderedNumbers, 3) ~= 3 then\n\
                     return 7\n\
                 end\n\
                 local reversed = reverse<<Int, {Int}>>(numbers)\n\
                 if List.get(reversed, 1) ~= 3 or List.get(reversed, 3) ~= 1 then\n\
                     return 3\n\
                 end\n\
                 local words: {String} = {\"a\", \"b\", \"c\"}\n\
                 if not containsBy<<String, {String}>>(words, \"b\", function(left: String, right: String): Boolean\n\
                     return left == right\n\
                 end) then\n\
                     return 4\n\
                 end\n\
                 local equalLeft: {Int} = {1, 2, 3}\n\
                 local equalRight: {Int} = {1, 4, 3}\n\
                 if equalsBy<<Int, {Int}, {Int}>>(equalLeft, equalRight, function(left: Int, right: Int): Boolean\n\
                     return left == right\n\
                 end) then\n\
                     return 5\n\
                 end\n\
                 local shortLeft: {Int} = {1}\n\
                 local shortRight: {Int} = {1, 2}\n\
                 if equalsBy<<Int, {Int}, {Int}>>(shortLeft, shortRight, function(left: Int, right: Int): Boolean\n\
                     return left == right\n\
                 end) then\n\
                     return 6\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("materializing Sequence consumer")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified materializing Sequence MIR")
            .call(entry, &[])
            .expect("materializing Sequence execution"),
        vec![int(42)]
    );
}

#[test]
fn deterministic_random_state_matches_the_frozen_stream() {
    let (mir, types) = executable_modules(&[
        (
            "src/random.pop",
            include_str!("../../../../libraries/standard/pop/src/random.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Random\n\
             public function verify(): Int\n\
                 local state = seed(1)\n\
                 if next(state) ~= 16807 or next(state) ~= 282475249 or next(state) ~= 1622650073 then\n\
                     return 1\n\
                 end\n\
                 if not verifyBytes() then\n\
                     return 2\n\
                 end\n\
                 if not verifySeeds() then\n\
                     return 3\n\
                 end\n\
                 local shuffleResult = verifyShuffle()\n\
                 if shuffleResult ~= 42 then\n\
                     return shuffleResult\n\
                 end\n\
                 if fill(seed(1), Bytes.create(), -1) then\n\
                     return 4\n\
                 end\n\
                 return 42\n\
             end\n\
             private function verifySeeds(): Boolean\n\
                 if next(seed(0)) ~= 16807 or next(seed(2147483647)) ~= 16807 or next(seed(4294967295)) ~= 16807 then\n\
                     return false\n\
                 end\n\
                 local unchanged = seed(1)\n\
                 if not fill(unchanged, Bytes.create(), 0) or next(unchanged) ~= 16807 then\n\
                     return false\n\
                 end\n\
                 local checkpoint = seed(1)\n\
                 local index = 0\n\
                 local value = 0\n\
                 while index < 10000 do\n\
                     value = Int(next(checkpoint))\n\
                     index += 1\n\
                 end\n\
                 return value == 1043618065\n\
             end\n\
             private function verifyBytes(): Boolean\n\
                 local output = Bytes.create()\n\
                 local bytesState = seed(1)\n\
                 if not fill(bytesState, output, 4) then\n\
                     return false\n\
                 end\n\
                 local snapshot = Bytes.toBytes(output)\n\
                 if Bytes.length(Bytes.view(snapshot)) ~= 4 or (Bytes.get(Bytes.view(snapshot), 1) ?? 0) ~= 166 or (Bytes.get(Bytes.view(snapshot), 4) ?? 0) ~= 41 then\n\
                     return false\n\
                 end\n\
                 return true\n\
             end\n\
             private function verifyShuffle(): Int\n\
                 local values: {Int} = {1, 2, 3, 4, 5}\n\
                 local shuffleState = seed(1)\n\
                 if not shuffle(shuffleState, values) then\n\
                     return 31\n\
                 end\n\
                 if Array.get(values, 1) ~= 4 then\n\
                     return 100 + Array.get(values, 1)\n\
                 end\n\
                 if Array.get(values, 2) ~= 3 then\n\
                     return 33\n\
                 end\n\
                 if Array.get(values, 3) ~= 5 then\n\
                     return 34\n\
                 end\n\
                 if Array.get(values, 4) ~= 1 or Array.get(values, 5) ~= 2 then\n\
                     return 35\n\
                 end\n\
                 if next(shuffleState) ~= 1144108930 then\n\
                     return 36\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("random consumer")
        .symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified random MIR");
    assert_eq!(
        interpreter
            .call(entry, &[])
            .expect("deterministic random execution"),
        vec![int(42)]
    );
}

#[test]
fn deterministic_random_distributions_are_bounded_and_unbiased() {
    let (mir, types) = executable_modules(&[
        (
            "src/random.pop",
            include_str!("../../../../libraries/standard/pop/src/random.pop"),
        ),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Random\n\
             public function verify(): Int\n\
                 local state = seed(1)\n\
                 if (nextInt(state, 10, 20) ?? 0) ~= 16 then\n\
                     return 1\n\
                 end\n\
                 if (nextInt(seed(1), -10, 10) ?? 0) ~= -4 or (nextInt(seed(1), 5, 6) ?? 0) ~= 5 then\n\
                     return 9\n\
                 end\n\
                 local wide = seed(1)\n\
                 if (nextInt(wide, 0, 3000000000) ?? -1) ~= 892629924 then\n\
                     return 2\n\
                 end\n\
                 local invalid = seed(1)\n\
                 if nextInt(invalid, 5, 5) ~= nil or nextInt(invalid, 7, 2) ~= nil then\n\
                     return 3\n\
                 end\n\
                 if nextInt(invalid, -9223372036854775807 - 1, 9223372036854775807) ~= nil or next(invalid) ~= 16807 then\n\
                     return 4\n\
                 end\n\
                 local floating = seed(1)\n\
                 local unit = nextFloat(floating)\n\
                 if unit < 0.0 or unit >= 1.0 or next(floating) ~= 282475249 then\n\
                     return 5\n\
                 end\n\
                 local probability = seed(1)\n\
                 if (chance(probability, 0.0) ?? true) or not (chance(probability, 1.0) ?? false) then\n\
                     return 6\n\
                 end\n\
                 if not (chance(probability, 0.5) ?? false) or next(probability) ~= 282475249 then\n\
                     return 7\n\
                 end\n\
                 if chance(probability, -0.1) ~= nil or chance(probability, 1.1) ~= nil then\n\
                     return 8\n\
                 end\n\
                 local nan = 0.0 / 0.0\n\
                 if chance(probability, nan) ~= nil then\n\
                     return 10\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("random distribution consumer")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified random distribution MIR")
            .call(entry, &[])
            .expect("deterministic random distribution execution"),
        vec![int(42)]
    );
}

#[test]
fn unicode_scalars_text_access_and_ascii_helpers_are_portable() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("backend crate is under the repository root");
    let unicode_source =
        std::fs::read_to_string(root.join("crates/libraries/standard/pop/src/unicode.pop"))
            .expect("read Pop.Unicode source");
    let (mir, types) = executable_modules(&[
        ("src/unicode.pop", unicode_source.as_str()),
        (
            "src/main.pop",
            "namespace Main\n\
             using Pop.Unicode\n\
             public function inspect(text: String): Int?\n\
                 local ascii = Text.get(text, 1)?\n\
                 local twoByte = Text.get(text, 2)?\n\
                 local threeByte = Text.get(text, 3)?\n\
                 local fourByte = Text.get(text, 4)?\n\
                 local final = Text.get(Text.slice(text, 2, 4), 4)?\n\
                 if Unicode.codePoint(ascii) ~= 65 or Unicode.codePoint(twoByte) ~= 233 or Unicode.codePoint(threeByte) ~= 20013 or Unicode.codePoint(fourByte) ~= 128512 or Unicode.codePoint(final) ~= 122 then\n\
                     return 1\n\
                 end\n\
                 local zeroIndex: Rune? = Text.get(text, 0)\n\
                 local negativeIndex: Rune? = Text.get(text, -1)\n\
                 local pastEnd: Rune? = Text.get(text, 6)\n\
                 if zeroIndex ~= nil or negativeIndex ~= nil or pastEnd ~= nil then\n\
                     return 2\n\
                 end\n\
                 local low = Unicode.fromCodePoint(0)?\n\
                 local beforeSurrogate = Unicode.fromCodePoint(55295)?\n\
                 local afterSurrogate = Unicode.fromCodePoint(57344)?\n\
                 local maximum = Unicode.fromCodePoint(1114111)?\n\
                 if Unicode.codePoint(low) ~= 0 or Unicode.codePoint(beforeSurrogate) ~= 55295 or Unicode.codePoint(afterSurrogate) ~= 57344 or Unicode.codePoint(maximum) ~= 1114111 then\n\
                     return 3\n\
                 end\n\
                 if Unicode.fromCodePoint(55296) ~= nil or Unicode.fromCodePoint(57343) ~= nil or Unicode.fromCodePoint(1114112) ~= nil then\n\
                     return 4\n\
                 end\n\
                 local upper = Unicode.fromCodePoint(65)?\n\
                 local lower = Unicode.fromCodePoint(122)?\n\
                 local digit = Unicode.fromCodePoint(57)?\n\
                 local space = Unicode.fromCodePoint(32)?\n\
                 if not isAscii(upper) or not isAsciiLetter(upper) or not isAsciiDigit(digit) or not isAsciiAlphanumeric(lower) or not isAsciiWhitespace(space) then\n\
                     return 5\n\
                 end\n\
                 if Unicode.codePoint(toAsciiLower(upper)) ~= 97 or Unicode.codePoint(toAsciiUpper(lower)) ~= 90 or toAsciiLower(fourByte) ~= fourByte then\n\
                     return 6\n\
                 end\n\
                 local asciiMaximum = Unicode.fromCodePoint(127)?\n\
                 local beyondAscii = Unicode.fromCodePoint(128)?\n\
                 if not isAscii(low) or not isAscii(asciiMaximum) or isAscii(beyondAscii) then\n\
                     return 7\n\
                 end\n\
                 local beforeUpper = Unicode.fromCodePoint(64)?\n\
                 local upperEnd = Unicode.fromCodePoint(90)?\n\
                 local afterUpper = Unicode.fromCodePoint(91)?\n\
                 local beforeLower = Unicode.fromCodePoint(96)?\n\
                 local lowerStart = Unicode.fromCodePoint(97)?\n\
                 local afterLower = Unicode.fromCodePoint(123)?\n\
                 if isAsciiLetter(beforeUpper) or not isAsciiLetter(upper) or not isAsciiLetter(upperEnd) or isAsciiLetter(afterUpper) or isAsciiLetter(beforeLower) or not isAsciiLetter(lowerStart) or not isAsciiLetter(lower) or isAsciiLetter(afterLower) then\n\
                     return 8\n\
                 end\n\
                 local beforeDigit = Unicode.fromCodePoint(47)?\n\
                 local digitStart = Unicode.fromCodePoint(48)?\n\
                 local afterDigit = Unicode.fromCodePoint(58)?\n\
                 if isAsciiDigit(beforeDigit) or not isAsciiDigit(digitStart) or not isAsciiDigit(digit) or isAsciiDigit(afterDigit) then\n\
                     return 9\n\
                 end\n\
                 if isAsciiAlphanumeric(beforeDigit) or not isAsciiAlphanumeric(digitStart) or not isAsciiAlphanumeric(digit) or isAsciiAlphanumeric(afterDigit) or isAsciiAlphanumeric(beforeUpper) or not isAsciiAlphanumeric(upper) or not isAsciiAlphanumeric(upperEnd) or isAsciiAlphanumeric(afterUpper) or isAsciiAlphanumeric(beforeLower) or not isAsciiAlphanumeric(lowerStart) or not isAsciiAlphanumeric(lower) or isAsciiAlphanumeric(afterLower) then\n\
                     return 10\n\
                 end\n\
                 local beforeTab = Unicode.fromCodePoint(8)?\n\
                 local tab = Unicode.fromCodePoint(9)?\n\
                 local carriageReturn = Unicode.fromCodePoint(13)?\n\
                 local afterCarriageReturn = Unicode.fromCodePoint(14)?\n\
                 local unitSeparator = Unicode.fromCodePoint(31)?\n\
                 local afterSpace = Unicode.fromCodePoint(33)?\n\
                 if isAsciiWhitespace(beforeTab) or not isAsciiWhitespace(tab) or not isAsciiWhitespace(carriageReturn) or isAsciiWhitespace(afterCarriageReturn) or isAsciiWhitespace(unitSeparator) or not isAsciiWhitespace(space) or isAsciiWhitespace(afterSpace) then\n\
                     return 11\n\
                 end\n\
                 if toAsciiLower(beforeUpper) ~= beforeUpper or Unicode.codePoint(toAsciiLower(upper)) ~= 97 or Unicode.codePoint(toAsciiLower(upperEnd)) ~= 122 or toAsciiLower(afterUpper) ~= afterUpper or toAsciiLower(beyondAscii) ~= beyondAscii then\n\
                     return 12\n\
                 end\n\
                 if toAsciiUpper(beforeLower) ~= beforeLower or Unicode.codePoint(toAsciiUpper(lowerStart)) ~= 65 or Unicode.codePoint(toAsciiUpper(lower)) ~= 90 or toAsciiUpper(afterLower) ~= afterLower or toAsciiUpper(beyondAscii) ~= beyondAscii then\n\
                     return 13\n\
                 end\n\
                 return 42\n\
             end\n",
        ),
    ]);
    let entry = mir.functions().last().expect("Unicode consumer").symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Unicode MIR");

    assert_eq!(
        interpreter
            .call(entry, &[MirValue::String("Aé中😀z".to_owned())])
            .expect("portable Unicode execution"),
        vec![int(42)]
    );
}

#[test]
fn rune_call_boundaries_reject_numeric_and_invalid_scalar_values() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function codePoint(value: Rune): UInt32\n\
             return Unicode.codePoint(value)\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified Rune MIR");

    assert_eq!(
        interpreter.call(SymbolId::from_raw(0), &[MirValue::Rune(65)]),
        Ok(vec![integer("65", IntegerKind::UInt32)])
    );
    assert_eq!(
        interpreter.call(SymbolId::from_raw(0), &[integer("65", IntegerKind::UInt32)]),
        Err(ExecutionError::TypeMismatch)
    );
    assert_eq!(
        interpreter.call(SymbolId::from_raw(0), &[MirValue::Rune(0xD800)]),
        Err(ExecutionError::TypeMismatch)
    );
}

#[test]
fn cleanup_resume_preserves_the_original_unwind_reason() {
    let mir = parse_mir_dump(concat!(
        "mir bubble b0 namespace n0\n",
        "dependencies\n",
        "function s0 f0() -> () effects[MayUnwind]\n",
        "  b0():\n",
        "    panic RuntimeInvariant\n",
        "function s1 f1() -> () effects[MayUnwind]\n",
        "  b0():\n",
        "    do v0 callDirect s0 () effects[MayUnwind] unwind cleanup:b1\n",
        "    return ()\n",
        "  b1() cleanup scope#1 reason unwind:\n",
        "    branch b2 ()\n",
        "  b2() cleanup scope#0 reason unwind:\n",
        "    resumeCurrentUnwind\n",
    ))
    .expect("cleanup MIR");
    let types = pop_types::TypeArena::new();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified cleanup MIR");

    assert!(matches!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Err(ExecutionError::Runtime(RuntimeFailure::Unwind(UnwindReason::Panic(payload))))
            if payload.kind() == PanicKind::RuntimeInvariant
    ));
}

#[test]
fn interpreter_rejects_foreign_calls_without_an_exact_typed_adapter() {
    let types = pop_types::TypeArena::new();
    let int32 = types.source_type("Int32").expect("Int32");
    let mir = parse_mir_dump(&format!(
        concat!(
            "mir bubble b0 namespace n0\n",
            "dependencies\n",
            "foreign s0 f0 params() results(t{int32}) symbol(native_poll) abi(C) links(-) effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks]\n",
            "function s1 f1() -> (t{int32}) effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks]\n",
            "  b0():\n",
            "    do v0 gcSafePoint sp0 roots ()\n",
            "    v1:t{int32} = callForeign s0 () safePoint sp0 roots () effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks] unwind propagate\n",
            "    return (v1)\n",
        ),
        int32 = int32.raw(),
    ))
    .expect("foreign MIR");
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified foreign MIR");

    assert_eq!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Err(ExecutionError::UnsupportedForeignFunction(
            SymbolId::from_raw(0)
        ))
    );
}

#[test]
fn interpreter_executes_only_an_exact_identity_and_signature_foreign_adapter() {
    let types = pop_types::TypeArena::new();
    let int32 = types.source_type("Int32").expect("Int32");
    let boolean = types.source_type("Boolean").expect("Boolean");
    let mir = parse_mir_dump(&format!(
        concat!(
            "mir bubble b0 namespace n0\n",
            "dependencies\n",
            "foreign s0 f0 params() results(t{int32}) symbol(native_poll) abi(C) links(-) effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks]\n",
            "function s1 f1() -> (t{int32}) effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks]\n",
            "  b0():\n",
            "    do v0 gcSafePoint sp0 roots ()\n",
            "    v1:t{int32} = callForeign s0 () safePoint sp0 roots () effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks] unwind propagate\n",
            "    return (v1)\n",
        ),
        int32 = int32.raw(),
    ))
    .expect("foreign MIR");

    let mismatched =
        TypedForeignAdapter::new(SymbolId::from_raw(0), Vec::new(), vec![boolean], |_| {
            Ok(vec![MirValue::Boolean(true)])
        });
    assert!(matches!(
        MirInterpreter::new(&mir, &types)
            .expect("verified foreign MIR")
            .with_foreign_adapter(mismatched),
        Err(ForeignAdapterRegistrationError::SignatureMismatch(symbol))
            if symbol == SymbolId::from_raw(0)
    ));

    let wrong_result =
        TypedForeignAdapter::new(SymbolId::from_raw(0), Vec::new(), vec![int32], |_| {
            Ok(vec![MirValue::Boolean(true)])
        });
    let wrong_result_interpreter = MirInterpreter::new(&mir, &types)
        .expect("verified foreign MIR")
        .with_foreign_adapter(wrong_result)
        .expect("declared adapter signature is exact");
    assert_eq!(
        wrong_result_interpreter.call(SymbolId::from_raw(1), &[]),
        Err(ExecutionError::TypeMismatch)
    );
    assert!(matches!(
        wrong_result_interpreter.runtime().events().last(),
        Some(ReferenceRuntimeEvent::LeaveForeign { .. })
    ));

    let adapter = TypedForeignAdapter::new(SymbolId::from_raw(0), Vec::new(), vec![int32], |_| {
        Ok(vec![MirValue::Integer(
            IntegerValue::parse_decimal("42", IntegerKind::Int32).expect("Int32"),
        )])
    });
    let interpreter = MirInterpreter::new(&mir, &types)
        .expect("verified foreign MIR")
        .with_foreign_adapter(adapter)
        .expect("exact typed adapter");
    assert_eq!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Ok(vec![MirValue::Integer(
            IntegerValue::parse_decimal("42", IntegerKind::Int32).expect("Int32")
        )])
    );
    assert!(matches!(
        interpreter.runtime().events(),
        [
            ReferenceRuntimeEvent::SafePoint { .. },
            ReferenceRuntimeEvent::SafePoint { .. },
            ReferenceRuntimeEvent::EnterForeign { .. },
            ReferenceRuntimeEvent::LeaveForeign { .. }
        ]
    ));
    assert!(matches!(
        interpreter.runtime().events().get(2),
        Some(ReferenceRuntimeEvent::EnterForeign {
            mode: pop_runtime_interface::ForeignCallMode::Blocking,
            ..
        })
    ));
}

#[test]
fn foreign_adapters_validate_closed_abi_values_not_only_declared_type_ids() {
    let mut types = pop_types::TypeArena::new();
    let int32 = types.source_type("Int32").expect("Int32");
    let pointer = types
        .intern(pop_types::SemanticType::Builtin {
            definition: pop_types::FFI_POINTER_TYPE_ID,
            arguments: vec![int32],
        })
        .expect("FFI pointer");
    let mir = parse_mir_dump(&format!(
        concat!(
            "mir bubble b0 namespace n0\n",
            "dependencies\n",
            "foreign s0 f0 params() results(t{pointer}) symbol(native_pointer) abi(C) links(-) effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks]\n",
            "function s1 f1() -> (t{pointer}) effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks]\n",
            "  b0():\n",
            "    do v0 gcSafePoint sp0 roots ()\n",
            "    v1:t{pointer} = callForeign s0 () safePoint sp0 roots () effects[ForeignFunction,UnsafeMemory,GcSafePoint,Blocks] unwind propagate\n",
            "    return (v1)\n",
        ),
        pointer = pointer.raw(),
    ))
    .expect("foreign pointer MIR");
    let adapter =
        TypedForeignAdapter::new(SymbolId::from_raw(0), Vec::new(), vec![pointer], |_| {
            Ok(vec![MirValue::Nil])
        });
    let interpreter = MirInterpreter::new(&mir, &types)
        .expect("verified foreign pointer MIR")
        .with_foreign_adapter(adapter)
        .expect("declared adapter signature is exact");

    assert_eq!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Err(ExecutionError::TypeMismatch)
    );
    assert!(matches!(
        interpreter.runtime().events().last(),
        Some(ReferenceRuntimeEvent::LeaveForeign { .. })
    ));
}

#[test]
fn panic_during_panic_cleanup_becomes_the_terminal_double_panic_kind() {
    let mir = parse_mir_dump(concat!(
        "mir bubble b0 namespace n0\n",
        "dependencies\n",
        "function s0 f0() -> () effects[MayUnwind]\n",
        "  b0():\n",
        "    panic RuntimeInvariant\n",
        "function s1 f1() -> () effects[MayUnwind]\n",
        "  b0():\n",
        "    do v0 callDirect s0 () effects[MayUnwind] unwind cleanup:b1\n",
        "    return ()\n",
        "  b1() cleanup scope#0 reason unwind:\n",
        "    do v1 callDirect s0 () effects[MayUnwind] unwind propagate\n",
        "    resumeCurrentUnwind\n",
    ))
    .expect("double-panic MIR");
    let types = pop_types::TypeArena::new();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified double-panic MIR");

    assert!(matches!(
        interpreter.call(SymbolId::from_raw(1), &[]),
        Err(ExecutionError::Runtime(RuntimeFailure::Unwind(UnwindReason::Panic(payload))))
            if payload.kind() == PanicKind::DoublePanic
    ));
}

#[test]
fn nominal_enum_cases_preserve_identity_and_equality() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public enum Color\n\
             Red\n\
             Blue\n\
         end\n\
         public function choose(flag: Boolean): Color\n\
             return if flag then Color.Red else Color.Blue\n\
         end\n\
         public function isRed(color: Color): Boolean\n\
             return color == Color.Red\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");
    let red = MirValue::Enum {
        definition: SymbolId::from_raw(0),
        case: EnumCaseId::from_raw(0),
        discriminant: 0,
    };
    let blue = MirValue::Enum {
        definition: SymbolId::from_raw(0),
        case: EnumCaseId::from_raw(1),
        discriminant: 1,
    };

    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[MirValue::Boolean(true)])
            .expect("red"),
        vec![red.clone()]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[MirValue::Boolean(false)])
            .expect("blue"),
        vec![blue.clone()]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[red])
            .expect("red equality"),
        vec![MirValue::Boolean(true)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[blue])
            .expect("blue inequality"),
        vec![MirValue::Boolean(false)]
    );
}

#[test]
fn fixed_packs_destructure_swap_and_preserve_target_before_value_order() {
    // ADR 0045: all target locations are evaluated once before RHS values,
    // then tuple projections are stored from left to right.
    let (mir, types) = executable_source(
        "namespace Main\n\
         public class Box\n\
             public value: Int = 1\n\
         end\n\
         private function split(value: Int): (Int, Int)\n\
             return value, value + 1\n\
         end\n\
         public function calculate(value: Int): Int\n\
             local left, right = split(value)\n\
             local result = split(value)\n\
             local projected = result[2]\n\
             left, right = right, left\n\
             local counter = 0\n\
             local function advance(): Int\n\
                 counter += 1\n\
                 return counter\n\
             end\n\
             local function observed(): Int\n\
                 return counter\n\
             end\n\
             local values: {Int} = { 10, 20 }\n\
             local box = Box {}\n\
             box.value, values[advance()], values[advance()] = 7, observed(), 99\n\
             return box.value * 100000 + projected * 10000 + right * 1000 + Array.get(values, 1) * 100 + Array.get(values, 2)\n\
         end\n",
    );
    let calculate = mir.functions().last().expect("calculate").symbol();
    let expected = vec![int(754_299)];
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");
    assert_eq!(
        interpreter.call(calculate, &[int(4)]).expect("fixed pack"),
        expected
    );

    let optimized = optimize_mir(mir.clone(), &types).expect("optimized fixed-pack MIR");
    let optimized_interpreter =
        MirInterpreter::new(&optimized, &types).expect("verified optimized MIR");
    assert_eq!(
        optimized_interpreter
            .call(calculate, &[int(4)])
            .expect("optimized fixed pack"),
        expected
    );
}

#[test]
fn typed_tables_lookup_replace_insert_and_preserve_insertion_order() {
    // ADR 0046: replacement keeps position and insertion appends.
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function build(): {[String]: Int}\n\
             local scores: {[String]: Int} = { alice = 10 }\n\
             scores[\"alice\"] = 11\n\
             scores[\"bruno\"] = 12\n\
             return scores\n\
         end\n\
         public function lookup(key: String): Int?\n\
             local scores: {[String]: Int} = { alice = 10 }\n\
             scores[\"bruno\"] = 12\n\
             return scores[key]\n\
         end\n",
    );
    let build = mir.functions()[0].symbol();
    let lookup = mir.functions()[1].symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");
    assert_eq!(
        interpreter.call(build, &[]).expect("table build"),
        vec![MirValue::Table(vec![
            (MirValue::String("alice".to_owned()), int(11)),
            (MirValue::String("bruno".to_owned()), int(12)),
        ])]
    );
    assert_eq!(
        interpreter
            .call(lookup, &[MirValue::String("bruno".to_owned())])
            .expect("present key"),
        vec![int(12)]
    );
    assert_eq!(
        interpreter
            .call(lookup, &[MirValue::String("missing".to_owned())])
            .expect("missing key"),
        vec![MirValue::Nil]
    );

    let optimized = optimize_mir(mir, &types).expect("optimized table MIR");
    let optimized_interpreter =
        MirInterpreter::new(&optimized, &types).expect("verified optimized MIR");
    assert_eq!(
        optimized_interpreter
            .call(lookup, &[MirValue::String("bruno".to_owned())])
            .expect("optimized present key"),
        vec![int(12)]
    );
}

#[test]
fn mutable_locals_flow_through_loop_backedges_and_branch_joins() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function calculate(doubleValue: Boolean): Int\n\
             local value = 0\n\
             while value < 10 do\n\
                 value = value + 1\n\
             end\n\
             if doubleValue then\n\
                 value = value + value\n\
             else\n\
                 value = value + 1\n\
             end\n\
             return value\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(function, &[MirValue::Boolean(true)])
            .expect("then branch"),
        vec![int(20)]
    );
    assert_eq!(
        interpreter
            .call(function, &[MirValue::Boolean(false)])
            .expect("else branch"),
        vec![int(11)]
    );
}

#[test]
fn repeat_until_executes_once_and_repeats_through_its_false_backedge() {
    // ADR 0060: the body runs before the first condition check, and `false`
    // returns to the body while `true` exits.
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function countToThree(): Int\n\
             local value = 0\n\
             repeat\n\
                 local nextValue = value + 1\n\
                 value = nextValue\n\
             until nextValue == 3\n\
             return value\n\
         end\n\
         public function runOnce(): Int\n\
             local value = 0\n\
             repeat\n\
                 value = value + 1\n\
             until true\n\
             return value\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified repeat-until MIR");

    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[])
            .expect("repeat backedge execution"),
        vec![int(3)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[])
            .expect("at-least-once execution"),
        vec![int(1)]
    );
}

#[test]
fn standard_print_overloads_execute_by_trusted_identity_and_return_no_value() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function run(): Int\n\
             print(42)\n\
             print(\"teste\")\n\
             print(\"\")\n\
             print(\"Pop 🫧\")\n\
             return 0\n\
         end\n",
    );
    assert!(mir.dump().contains("callStandard sf0"));
    assert!(mir.dump().contains("callStandard sf1"));
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(mir.functions()[0].symbol(), &[])
            .expect("standard print call"),
        vec![int(0)]
    );
}

#[test]
fn declared_functions_flow_through_typed_values_and_indirect_calls() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private function increment(value: Int): Int\n\
             return 42\n\
         end\n\
         private function apply(operation: function(value: Int): Int, value: Int): Int\n\
             return operation(value)\n\
         end\n\
         public function run(value: Int): Int\n\
             local operation: function(value: Int): Int = increment\n\
             return apply(operation, value)\n\
         end\n",
    );
    let run = mir.functions()[2].symbol();

    assert!(mir.dump().contains("callIndirect"));
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(run, &[int(41)])
            .expect("indirect call"),
        vec![int(42)]
    );
}

#[test]
fn integer_overflow_and_division_by_zero_are_deterministic_traps() {
    for (operator, expected) in [
        ("+", trap(TrapKind::IntegerOverflow)),
        ("/", trap(TrapKind::DivisionByZero)),
    ] {
        let source = format!(
            "namespace Main\n\
             public function calculate(left: Int, right: Int): Int\n\
                 return left {operator} right\n\
             end\n"
        );
        let (mir, types) = executable_source(&source);
        let function = mir.functions()[0].symbol();
        let arguments = if operator == "+" {
            [int(i64::MAX), int(1)]
        } else {
            [int(1), int(0)]
        };
        let error = MirInterpreter::new(&mir, &types)
            .expect("verified")
            .call(function, &arguments)
            .expect_err("trap");
        assert_eq!(error, expected);
    }
}

#[test]
fn tuples_records_unions_and_false_loops_share_one_mir_runtime_model() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public record Score\n\
             value: Int\n\
         end\n\
         public union State\n\
             Idle\n\
             Ready(score: Score)\n\
         end\n\
         public function increment(score: Score): Score\n\
             while false do\n\
                 score.value\n\
             end\n\
             return score with { value = score.value + 1, }\n\
         end\n\
         public function pair(): (Int, String)\n\
             return (7, \"ready\")\n\
         end\n\
         public function idle(): State\n\
             return State.Idle\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");
    let increment = mir.functions()[0].symbol();
    let pair = mir.functions()[1].symbol();
    let idle = mir.functions()[2].symbol();

    assert_eq!(
        interpreter
            .call(
                increment,
                &[MirValue::Record {
                    record: SymbolId::from_raw(0),
                    fields: vec![(FieldId::from_raw(0), int(4))],
                }],
            )
            .expect("record update"),
        vec![MirValue::Record {
            record: SymbolId::from_raw(0),
            fields: vec![(FieldId::from_raw(0), int(5))],
        }]
    );
    assert_eq!(
        interpreter.call(pair, &[]).expect("tuple"),
        vec![MirValue::Tuple(vec![
            int(7),
            MirValue::String("ready".to_owned()),
        ])]
    );
    assert_eq!(
        interpreter.call(idle, &[]).expect("union"),
        vec![MirValue::Union {
            union: SymbolId::from_raw(1),
            case: UnionCaseId::from_raw(0),
            arguments: Vec::new(),
        }]
    );
}

#[test]
fn omitted_record_defaults_execute_as_complete_typed_values() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public record Options\n\
             name: String\n\
             attempts: Int = 3\n\
             enabled: Boolean = true\n\
         end\n\
         public function defaults(): (Int, Boolean)\n\
             local options: Options = { name = \"pop\", }\n\
             return (options.attempts, options.enabled)\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();

    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(function, &[])
            .expect("record defaults"),
        vec![MirValue::Tuple(vec![int(3), MirValue::Boolean(true),])]
    );
}

#[test]
fn structural_records_keep_named_defaults_and_ignore_initializer_field_order_in_equality() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public record First\n\
             value: Int = 1\n\
         end\n\
         public record Second\n\
             value: Int = 2\n\
         end\n\
         public record Pair\n\
             left: Int\n\
             right: Int\n\
         end\n\
         public function first(): Int\n\
             local value: First = {}\n\
             return value.value\n\
         end\n\
         public function second(): Int\n\
             local value: Second = {}\n\
             return value.value\n\
         end\n\
         public function equalInAnyOrder(): Boolean\n\
             local first: Pair = { left = 1, right = 2, }\n\
             local second: Pair = { right = 2, left = 1, }\n\
             return first == second\n\
         end\n\
         private function secondArgument(value: Second): Int\n\
             return value.value\n\
         end\n\
         public function callSecondWithDefault(): Int\n\
             return secondArgument({})\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[])
            .expect("First default"),
        vec![int(1)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[])
            .expect("Second default"),
        vec![int(2)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[2].symbol(), &[])
            .expect("structural equality"),
        vec![MirValue::Boolean(true)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[4].symbol(), &[])
            .expect("named parameter default"),
        vec![int(2)]
    );
}

#[test]
fn arrays_and_tables_execute_identically_before_and_after_mir_optimization() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function collections(): ({String}, {[String]: Int})\n\
             local names: {String} = { \"first\", \"second\" }\n\
             local scores: {[String]: Int} = { first = 1, second = 2 }\n\
             names[2] = \"updated\"\n\
             return (names, scores)\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    let optimized = optimize_mir(mir.clone(), &types).expect("optimized MIR");
    let expected = vec![MirValue::Tuple(vec![
        MirValue::Array(vec![
            MirValue::String("first".to_owned()),
            MirValue::String("updated".to_owned()),
        ]),
        MirValue::Table(vec![
            (MirValue::String("first".to_owned()), int(1)),
            (MirValue::String("second".to_owned()), int(2)),
        ]),
    ])];

    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(function, &[])
            .expect("collections"),
        expected
    );
    assert_eq!(
        MirInterpreter::new(&optimized, &types)
            .expect("verified optimized MIR")
            .call(function, &[])
            .expect("optimized collections"),
        expected
    );
}

#[test]
fn managed_array_mutation_through_a_call_preserves_identity() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private function replaceFirst(values: {Int})\n\
             values[1] = 42\n\
         end\n\
         public function verify(): Int\n\
             local values: {Int} = {1, 2}\n\
             replaceFirst(values)\n\
             return Array.get(values, 1)\n\
         end\n",
    );
    let verify = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("array identity consumer")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified array identity MIR")
            .call(verify, &[])
            .expect("array mutation through call"),
        vec![int(42)]
    );
}

#[test]
fn array_indexing_is_one_based_and_returns_nil_out_of_bounds() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function at(values: {String}, index: Int): String?\n\
             return values[index]\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");
    let values = MirValue::Array(vec![
        MirValue::String("first".to_owned()),
        MirValue::String("second".to_owned()),
    ]);

    assert_eq!(
        interpreter
            .call(function, &[values.clone(), int(1)])
            .expect("first element"),
        vec![MirValue::String("first".to_owned())]
    );
    assert_eq!(
        interpreter
            .call(function, &[values.clone(), int(0)])
            .expect("zero index"),
        vec![MirValue::Nil]
    );
    assert_eq!(
        interpreter
            .call(function, &[values, int(3)])
            .expect("past the end"),
        vec![MirValue::Nil]
    );
}

#[test]
fn fixed_array_core_operations_execute_with_one_based_checked_semantics() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function arrays(): (Int, Int, Int?)\n\
             local values = Array.create<<Int>>(4, 0)\n\
             Array.fill(values, 7)\n\
             values[1] = 3\n\
             return (Array.length(values), Array.get(values, 1), values[5])\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    let expected = vec![MirValue::Tuple(vec![int(4), int(3), MirValue::Nil])];

    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(function, &[])
            .expect("array core operations"),
        expected
    );
    let optimized = optimize_mir(mir, &types).expect("optimized MIR");
    assert_eq!(
        MirInterpreter::new(&optimized, &types)
            .expect("verified optimized MIR")
            .call(function, &[])
            .expect("optimized array core operations"),
        expected
    );
}

#[test]
fn fixed_array_negative_lengths_and_checked_bounds_trap() {
    for source in [
        "namespace Main\npublic function fail(): Int\nlocal values = Array.create<<Int>>(-1, 0)\nreturn 0\nend\n",
        "namespace Main\npublic function fail(): Int\nlocal values = Array.create<<Int>>(1, 0)\nreturn Array.get(values, 2)\nend\n",
    ] {
        let (mir, types) = executable_source(source);
        let function = mir.functions()[0].symbol();
        assert!(matches!(
            MirInterpreter::new(&mir, &types)
                .expect("verified MIR")
                .call(function, &[]),
            Err(ExecutionError::Runtime(RuntimeFailure::Trap(trap)))
                if trap.kind() == TrapKind::BoundsViolation
        ));
    }
}

#[test]
fn growable_lists_execute_with_stable_order_and_generalized_iteration() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function lists(): (Int, Int, Int?, Int)\n\
             local values = List.withCapacity<<Int>>(1)\n\
             List.add(values, 0)\n\
             List.add(values, 42)\n\
             values[1] = 3\n\
             local total = 0\n\
             for value in values do\n\
                 total += value\n\
             end\n\
             return (List.length(values), List.get(values, 1), values[3], total)\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    let expected = vec![MirValue::Tuple(vec![
        int(2),
        int(3),
        MirValue::Nil,
        int(45),
    ])];
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(function, &[])
            .expect("list core operations"),
        expected
    );
    let optimized = optimize_mir(mir, &types).expect("optimized MIR");
    assert_eq!(
        MirInterpreter::new(&optimized, &types)
            .expect("verified optimized MIR")
            .call(function, &[])
            .expect("optimized list core operations"),
        expected
    );
}

#[test]
fn first_class_integer_ranges_execute_in_both_directions() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function ranges(): Int\n\
             local total = 0\n\
             for value in Range.create(1, 5, 2) do\n\
                 total += value\n\
             end\n\
             for value in Range.create(5, 1, -2) do\n\
                 total += value\n\
             end\n\
             for value in Range.create(5, 1) do\n\
                 total += 100\n\
             end\n\
             return total\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified range MIR")
            .call(function, &[])
            .expect("range execution"),
        vec![int(18)]
    );
}

#[test]
fn first_class_ranges_are_repeatable_and_preserve_traps() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function repeatRange(): Int\n\
             local values = Range.create(1, 3)\n\
             local total = 0\n\
             for value in values do\n\
                 total += value\n\
             end\n\
             for value in values do\n\
                 total += value\n\
             end\n\
             return total\n\
         end\n\
         public function dynamicZero(step: Int): Int\n\
             for value in Range.create(1, 3, step) do\n\
                 return value\n\
             end\n\
             return 0\n\
         end\n\
         public function overflow(first: Int8, last: Int8, step: Int8): Int\n\
             local total = 0\n\
             for value in Range.create(first, last, step) do\n\
                 total += Int(value)\n\
             end\n\
             return total\n\
         end\n\
         public function breakBeforeOverflow(first: Int8, last: Int8, step: Int8): Int\n\
             local total = 0\n\
             for value in Range.create(first, last, step) do\n\
                 total += Int(value)\n\
                 break\n\
             end\n\
             return total\n\
         end\n\
         public function evaluateRangeArgumentsOnce(): Int\n\
             local calls = 0\n\
             local nextValue = function(): Int\n\
                 calls += 1\n\
                 return calls\n\
             end\n\
             local total = 0\n\
             for value in Range.create(nextValue(), nextValue(), nextValue()) do\n\
                 total += value\n\
             end\n\
             return calls * 10 + total\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified range traps MIR");
    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[])
            .expect("independent range iterators"),
        vec![int(12)]
    );
    assert_eq!(
        interpreter.call(mir.functions()[1].symbol(), &[int(0)]),
        Err(trap(TrapKind::InvalidRangeStep))
    );
    assert_eq!(
        interpreter.call(
            mir.functions()[2].symbol(),
            &[
                integer("126", IntegerKind::Int8),
                integer("127", IntegerKind::Int8),
                integer("2", IntegerKind::Int8),
            ],
        ),
        Err(trap(TrapKind::IntegerOverflow))
    );
    let int8_arguments = [
        integer("126", IntegerKind::Int8),
        integer("127", IntegerKind::Int8),
        integer("2", IntegerKind::Int8),
    ];
    assert_eq!(
        interpreter
            .call(mir.functions()[3].symbol(), &int8_arguments)
            .expect("break avoids unused advancement"),
        vec![int(126)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[4].symbol(), &[])
            .expect("range arguments evaluate once"),
        vec![int(31)]
    );
}

#[test]
fn generalized_iteration_cleanup_is_explicit_and_lexical() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private class ResourceIterator implements Iterator<Int>\n\
             private current: Int\n\
             private closed: Boolean\n\
             public function ResourceIterator.new(): ResourceIterator\n\
                 return ResourceIterator { current = 1, closed = false }\n\
             end\n\
             public function ResourceIterator:iterator(): Iterator<Int>\n\
                 return self\n\
             end\n\
             public function ResourceIterator:next(): Iteration<Int>\n\
                 if self.current > 2 then\n\
                     return Iteration.End\n\
                 end\n\
                 local value = self.current\n\
                 self.current += 1\n\
                 return Iteration.Item(value)\n\
             end\n\
             public function ResourceIterator:close()\n\
                 self.closed = true\n\
             end\n\
             public function ResourceIterator:isClosed(): Boolean\n\
                 return self.closed\n\
             end\n\
         end\n\
         private function consumeWithCleanup(iterator: ResourceIterator): Boolean\n\
             defer\n\
                 iterator:close()\n\
             end\n\
             for value in iterator do\n\
                 break\n\
             end\n\
             return iterator:isClosed()\n\
         end\n\
         public function cleanupContract(): (Boolean, Boolean, Boolean)\n\
             local withoutCleanup = ResourceIterator.new()\n\
             for value in withoutCleanup do\n\
                 break\n\
             end\n\
             local withCleanup = ResourceIterator.new()\n\
             local closedBeforeReturn = consumeWithCleanup(withCleanup)\n\
             return (withoutCleanup:isClosed(), closedBeforeReturn, withCleanup:isClosed())\n\
         end\n",
    );
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("cleanup contract")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified explicit cleanup MIR")
            .call(function, &[])
            .expect("explicit cleanup execution"),
        vec![MirValue::Tuple(vec![
            MirValue::Boolean(false),
            MirValue::Boolean(false),
            MirValue::Boolean(true),
        ])]
    );
}

#[test]
fn generalized_iteration_acquires_and_steps_exactly_once() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private class CountingIterator implements Iterator<Int>\n\
             private current: Int\n\
             private limit: Int\n\
             private nextCalls: Int\n\
             public function CountingIterator.new(limit: Int): CountingIterator\n\
                 return CountingIterator { current = 1, limit = limit, nextCalls = 0 }\n\
             end\n\
             public function CountingIterator:iterator(): Iterator<Int>\n\
                 return self\n\
             end\n\
             public function CountingIterator:next(): Iteration<Int>\n\
                 self.nextCalls += 1\n\
                 if self.current > self.limit then\n\
                     return Iteration.End\n\
                 end\n\
                 local value = self.current\n\
                 self.current += 1\n\
                 return Iteration.Item(value)\n\
             end\n\
             public function CountingIterator:code(total: Int): Int\n\
                 return self.nextCalls * 10 + total\n\
             end\n\
         end\n\
         public function iterationCounts(): (Int, Int, Int, Int)\n\
             local empty = CountingIterator.new(0)\n\
             for value in empty do\n\
             end\n\
             local single = CountingIterator.new(3)\n\
             local singleTotal = 0\n\
             for value in single do\n\
                 singleTotal += value\n\
                 break\n\
             end\n\
             local multiple = CountingIterator.new(2)\n\
             local multipleTotal = 0\n\
             for value in multiple do\n\
                 multipleTotal += value\n\
             end\n\
             local nestedTotal = 0\n\
             for outer in Range.create(1, 2) do\n\
                 for inner in Range.create(1, 2) do\n\
                     if inner == 1 then\n\
                         continue\n\
                     end\n\
                     nestedTotal += outer * inner\n\
                 end\n\
             end\n\
             return (empty:code(0), single:code(singleTotal), multiple:code(multipleTotal), nestedTotal)\n\
         end\n",
    );
    let function = mir
        .functions()
        .iter()
        .find(|function| function.parameters().is_empty())
        .expect("iteration counts")
        .symbol();
    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified iteration count MIR")
            .call(function, &[])
            .expect("iteration call counts"),
        vec![MirValue::Tuple(vec![int(10), int(11), int(33), int(6)])]
    );
}

#[test]
fn growable_list_negative_capacity_and_checked_bounds_trap() {
    for source in [
        "namespace Main\npublic function fail(): Int\nlocal values = List.withCapacity<<Int>>(-1)\nreturn 0\nend\n",
        "namespace Main\npublic function fail(): Int\nlocal values = List.create<<Int>>()\nreturn List.get(values, 1)\nend\n",
    ] {
        let (mir, types) = executable_source(source);
        let function = mir.functions()[0].symbol();
        assert!(matches!(
            MirInterpreter::new(&mir, &types)
                .expect("verified MIR")
                .call(function, &[]),
            Err(ExecutionError::Runtime(RuntimeFailure::Trap(trap)))
                if trap.kind() == TrapKind::BoundsViolation
        ));
    }
}

#[test]
fn native_class_construction_and_resolved_fields_execute_without_tables() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public class Counter\n\
             public value: Int\n\
             public step: Int = 2\n\
             public function Counter.new(value: Int): Counter\n\
                 return Counter { value = value }\n\
             end\n\
             public function Counter:add(delta: Int): Counter\n\
                 self.value = self.value + delta\n\
                 return self\n\
             end\n\
             public function Counter:get(): Int\n\
                 return self.value + self.step\n\
             end\n\
         end\n\
         public function read(value: Int): Int\n\
             local counter = Counter.new(value)\n\
             counter:add(3)\n\
             return counter:get()\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();

    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(function, &[int(7)])
            .expect("class construction"),
        vec![int(12)]
    );
}

#[test]
fn equality_preserves_value_and_native_class_identity_semantics() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public record Point\n\
             x: Int\n\
             name: String\n\
         end\n\
         public class Token\n\
             public value: Int\n\
         end\n\
         public function compare(value: Int): (Boolean, Boolean, Boolean, Boolean, Boolean, Boolean)\n\
             local left: Point = { x = value, name = \"pop\" }\n\
             local right: Point = { x = value, name = \"pop\" }\n\
             local first = Token { value = value }\n\
             local alias = first\n\
             local other = Token { value = value }\n\
             return (value == 7, \"pop\" ~= \"lua\", left == right, (1, \"x\") == (1, \"x\"), first == alias, first ~= other)\n\
         end\n",
    );
    let function = mir.functions()[0].symbol();

    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(function, &[int(7)])
            .expect("equality"),
        vec![MirValue::Tuple(vec![MirValue::Boolean(true); 6])]
    );
}

#[test]
fn logical_operators_short_circuit_before_trapping_right_operands() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private function trap(): Boolean\n\
             return 1 / 0 > 0\n\
         end\n\
         public function falseAnd(): Boolean\n\
             return false and trap()\n\
         end\n\
         public function trueOr(): Boolean\n\
             return true or trap()\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[])
            .expect("false and short-circuits"),
        vec![MirValue::Boolean(false)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[2].symbol(), &[])
            .expect("true or short-circuits"),
        vec![MirValue::Boolean(true)]
    );
}

#[test]
fn optional_flow_distinguishes_absent_from_present_false_and_zero() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function choose(value: Int?, fallback: Int): Int\n\
             return value ?? fallback\n\
         end\n\
         public function isPresent(value: Boolean?): Int\n\
             if local present = value then\n\
                 return 1\n\
             end\n\
             return 0\n\
         end\n\
         public function propagate(value: Int?): Int?\n\
             value?\n\
             return value\n\
         end\n\
         private function trapDefault(): Int\n\
             return 1 / 0\n\
         end\n\
         public function lazy(value: Int?): Int\n\
             return value ?? trapDefault()\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified optional MIR");

    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[int(0), int(7)])
            .expect("present zero"),
        vec![int(0)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[0].symbol(), &[MirValue::Nil, int(7)])
            .expect("absent default"),
        vec![int(7)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[MirValue::Boolean(false)],)
            .expect("present false"),
        vec![int(1)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[1].symbol(), &[MirValue::Nil])
            .expect("absent Boolean"),
        vec![int(0)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[2].symbol(), &[MirValue::Nil])
            .expect("propagated absence"),
        vec![MirValue::Nil]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[2].symbol(), &[int(0)])
            .expect("propagated presence"),
        vec![int(0)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[4].symbol(), &[int(0)])
            .expect("present value skips fallback"),
        vec![int(0)]
    );
    assert_eq!(
        interpreter
            .call(mir.functions()[4].symbol(), &[MirValue::Nil])
            .expect_err("absent value evaluates fallback"),
        trap(TrapKind::DivisionByZero)
    );

    let optimized = optimize_mir(mir, &types).expect("optimized optional MIR");
    let optimized_interpreter =
        MirInterpreter::new(&optimized, &types).expect("verified optimized optional MIR");
    assert_eq!(
        optimized_interpreter
            .call(optimized.functions()[0].symbol(), &[int(0), int(7)])
            .expect("optimized present zero"),
        vec![int(0)]
    );
    assert_eq!(
        optimized_interpreter
            .call(optimized.functions()[0].symbol(), &[MirValue::Nil, int(7)])
            .expect("optimized absent default"),
        vec![int(7)]
    );
}

#[test]
fn zero_result_calls_execute_for_every_resolved_dispatch_kind() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         private function observe(value: Int)\n\
             value\n\
         end\n\
         private function apply(operation: function(value: Int), value: Int)\n\
             operation(value)\n\
         end\n\
         public class Connection\n\
             private closed: Boolean = false\n\
             public function Connection:close()\n\
                 self.closed = true\n\
             end\n\
             public function Connection:isClosed(): Boolean\n\
                 return self.closed\n\
             end\n\
             public function Connection.reopen(connection: Connection)\n\
                 connection.closed = false\n\
             end\n\
         end\n\
         public function run(): Boolean\n\
             local operation: function(value: Int) = observe\n\
             apply(operation, 1)\n\
             operation(2)\n\
             local connection = Connection {}\n\
             connection:close()\n\
             Connection.reopen(connection)\n\
             connection:close()\n\
             return connection:isClosed()\n\
         end\n",
    );
    let run = mir.functions()[2].symbol();

    assert_eq!(
        MirInterpreter::new(&mir, &types)
            .expect("verified MIR")
            .call(run, &[])
            .expect("zero-result calls"),
        vec![MirValue::Boolean(true)]
    );
}

fn integer(text: &str, kind: IntegerKind) -> MirValue {
    MirValue::Integer(IntegerValue::parse_decimal(text, kind).expect("integer test value"))
}

fn int(value: i64) -> MirValue {
    integer(&value.to_string(), IntegerKind::Int64)
}

fn float(text: &str, kind: FloatKind) -> MirValue {
    MirValue::Float(FloatValue::parse_decimal(text, kind).expect("float test value"))
}

#[test]
fn exact_numeric_kinds_execute_checked_and_ieee_semantics() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function addByte(left: UInt8, right: UInt8): UInt8\n\
             return left + right\n\
         end\n\
         public function lessUnsigned(left: UInt64, right: UInt64): Boolean\n\
             return left < right\n\
         end\n\
         public function addSingle(left: Float32, right: Float32): Float32\n\
             return left + right\n\
         end\n\
         public function divideDouble(left: Float64, right: Float64): Float64\n\
             return left / right\n\
         end\n\
         public function identityByte(value: UInt8): UInt8\n\
             return value\n\
         end\n\
         public function identitySingle(value: Float32): Float32\n\
             return value\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(
                mir.functions()[0].symbol(),
                &[
                    integer("254", IntegerKind::UInt8),
                    integer("1", IntegerKind::UInt8),
                ],
            )
            .expect("UInt8 add"),
        vec![integer("255", IntegerKind::UInt8)]
    );
    assert_eq!(
        interpreter.call(
            mir.functions()[0].symbol(),
            &[
                integer("255", IntegerKind::UInt8),
                integer("1", IntegerKind::UInt8),
            ],
        ),
        Err(trap(TrapKind::IntegerOverflow))
    );
    assert_eq!(
        interpreter
            .call(
                mir.functions()[1].symbol(),
                &[
                    integer("9223372036854775808", IntegerKind::UInt64),
                    integer("18446744073709551615", IntegerKind::UInt64),
                ],
            )
            .expect("UInt64 comparison"),
        vec![MirValue::Boolean(true)]
    );

    let single = interpreter
        .call(
            mir.functions()[2].symbol(),
            &[
                float("16777216", FloatKind::Float32),
                float("1", FloatKind::Float32),
            ],
        )
        .expect("Float32 rounding");
    assert_eq!(single, vec![float("16777216", FloatKind::Float32)]);

    let divided = interpreter
        .call(
            mir.functions()[3].symbol(),
            &[
                float("1", FloatKind::Float64),
                float("0", FloatKind::Float64),
            ],
        )
        .expect("IEEE zero division");
    let MirValue::Float(value) = divided[0] else {
        panic!("float result");
    };
    assert!(value.as_f64().is_infinite());

    assert_eq!(
        interpreter.call(
            mir.functions()[4].symbol(),
            &[integer("1", IntegerKind::Int16)],
        ),
        Err(ExecutionError::TypeMismatch)
    );
    assert_eq!(
        interpreter.call(
            mir.functions()[5].symbol(),
            &[float("1", FloatKind::Float64)],
        ),
        Err(ExecutionError::TypeMismatch)
    );
}

#[test]
fn remaining_exact_numeric_operations_preserve_width_and_format() {
    let (mir, types) = executable_source(
        "namespace Main\n\
         public function integerOperations(left: Int16, right: Int16): (Int16, Int16, Int16, Int16, Int16)\n\
             return (left - right, left * right, left / right, left % right, -left)\n\
         end\n\
         public function floatOperations(left: Float64, right: Float64): (Float64, Float64, Float64, Boolean, Boolean)\n\
             return (left - right, left * right, -left, left < right, left > right)\n\
         end\n",
    );
    let interpreter = MirInterpreter::new(&mir, &types).expect("verified MIR");

    assert_eq!(
        interpreter
            .call(
                mir.functions()[0].symbol(),
                &[
                    integer("7", IntegerKind::Int16),
                    integer("2", IntegerKind::Int16),
                ],
            )
            .expect("remaining integer operations"),
        vec![MirValue::Tuple(vec![
            integer("5", IntegerKind::Int16),
            integer("14", IntegerKind::Int16),
            integer("3", IntegerKind::Int16),
            integer("1", IntegerKind::Int16),
            integer("-7", IntegerKind::Int16),
        ])]
    );
    assert_eq!(
        interpreter
            .call(
                mir.functions()[1].symbol(),
                &[
                    float("6", FloatKind::Float64),
                    float("2", FloatKind::Float64),
                ],
            )
            .expect("remaining float operations"),
        vec![MirValue::Tuple(vec![
            float("4", FloatKind::Float64),
            float("12", FloatKind::Float64),
            float("-6", FloatKind::Float64),
            MirValue::Boolean(false),
            MirValue::Boolean(true),
        ])]
    );
}
