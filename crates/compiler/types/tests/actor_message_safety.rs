use pop_foundation::{BubbleId, FileId, ModuleId};
use pop_resolve::{ModuleInput, ResolutionDatabase, SymbolSpace, build_declaration_index};
use pop_source::SourceFile;
use pop_syntax::{
    NodeKind, parse_file, parse_function_signature, parse_record_declaration,
    parse_union_declaration,
};
use pop_types::{
    ActorMessageSafety, ActorMessageUnsafeKind, SignatureResolver, embedded_bootstrap_schema,
};

#[test]
#[allow(clippy::too_many_lines)]
fn actor_message_safety_is_recursive_and_rejects_mutable_or_executable_values() {
    let module = ModuleId::from_raw(0);
    let source = SourceFile::new(
        FileId::from_raw(0),
        "src/messages.pop",
        "namespace Example\n\
         public record Point\n\
             x: Int\n\
             label: String\n\
         end\n\
         public union Message\n\
             Move(point: Point)\n\
             Rename(label: String)\n\
         end\n\
         public function inspect<T>(\n\
             message: Message,\n\
             point: Point,\n\
             pair: (Int, String),\n\
             actor: Actor.Ref<Message>,\n\
             reply: Actor.Reply<Point>,\n\
             inbox: Actor.Inbox<Message>,\n\
             values: {Int},\n\
             entries: {[String]: Int},\n\
             callback: function(value: Int): Int,\n\
             unresolved: T,\n\
         )\n\
         end\n",
    )
    .expect("source");
    let syntax = parse_file(&source);
    let indexed = build_declaration_index(&[ModuleInput::new(
        module,
        BubbleId::from_raw(0),
        &source,
        &syntax,
    )]);
    let record_symbol = indexed
        .index()
        .declaration_by_qualified_name("Example.Point", SymbolSpace::Type)[0]
        .symbol();
    let union_symbol = indexed
        .index()
        .declaration_by_qualified_name("Example.Message", SymbolSpace::Type)[0]
        .symbol();
    let function_symbol = indexed
        .index()
        .declaration_by_qualified_name("Example.inspect", SymbolSpace::Value)[0]
        .symbol();
    let database = ResolutionDatabase::new(indexed.into_index());
    let mut resolver =
        SignatureResolver::new(&database, embedded_bootstrap_schema().expect("bootstrap"));

    let record_node = syntax
        .root()
        .children()
        .iter()
        .find(|node| node.kind() == NodeKind::RecordDeclaration)
        .expect("record");
    let record = parse_record_declaration(&source, &syntax, record_node).expect("record syntax");
    assert!(
        resolver
            .define_record(module, record_symbol, &record)
            .diagnostics()
            .is_empty()
    );

    let union_node = syntax
        .root()
        .children()
        .iter()
        .find(|node| node.kind() == NodeKind::UnionDeclaration)
        .expect("union");
    let union = parse_union_declaration(&source, &syntax, union_node).expect("union syntax");
    assert!(
        resolver
            .define_union(module, union_symbol, &union)
            .diagnostics()
            .is_empty()
    );

    let function_node = syntax
        .root()
        .children()
        .iter()
        .find(|node| node.kind() == NodeKind::FunctionDeclaration)
        .expect("function");
    let function =
        parse_function_signature(&source, &syntax, function_node).expect("function syntax");
    let signature_result = resolver.resolve(module, function_symbol, &function);
    assert!(
        signature_result.diagnostics().is_empty(),
        "{}",
        signature_result.diagnostic_snapshot()
    );
    let signature = signature_result.signature().expect("signature");
    let parameter_type = |name: &str| {
        signature
            .parameters()
            .iter()
            .find(|parameter| parameter.name() == name)
            .and_then(|parameter| parameter.parameter_type().type_id())
            .expect("parameter type")
    };

    for name in ["message", "point", "pair", "actor", "reply"] {
        assert_eq!(
            resolver.actor_message_safety(parameter_type(name)),
            ActorMessageSafety::Safe,
            "{name}"
        );
    }
    for (name, kind) in [
        ("inbox", ActorMessageUnsafeKind::Builtin),
        ("values", ActorMessageUnsafeKind::MutableCollection),
        ("entries", ActorMessageUnsafeKind::MutableCollection),
        ("callback", ActorMessageUnsafeKind::Callable),
        ("unresolved", ActorMessageUnsafeKind::TypeParameter),
    ] {
        assert!(matches!(
            resolver.actor_message_safety(parameter_type(name)),
            ActorMessageSafety::Unsafe {
                kind: actual,
                ..
            } if actual == kind
        ));
    }
}
