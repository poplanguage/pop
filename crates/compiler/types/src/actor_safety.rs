use std::collections::BTreeSet;

use pop_foundation::TypeId;

use crate::{ACTOR_REF_TYPE_ID, ACTOR_REPLY_TYPE_ID, SemanticType, SignatureResolver};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorMessageUnsafeKind {
    UnknownType,
    MutableCollection,
    Callable,
    Class,
    Interface,
    Attribute,
    Builtin,
    TypeParameter,
    Opaque,
    ErrorUnion,
    CompilerError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorMessageSafety {
    Safe,
    Unsafe {
        type_id: TypeId,
        kind: ActorMessageUnsafeKind,
    },
}

impl SignatureResolver<'_> {
    #[must_use]
    pub fn actor_message_safety(&self, type_id: TypeId) -> ActorMessageSafety {
        actor_message_safety(self, type_id, &mut BTreeSet::new(), &mut BTreeSet::new())
    }
}

fn actor_message_safety(
    resolver: &SignatureResolver<'_>,
    type_id: TypeId,
    visiting: &mut BTreeSet<TypeId>,
    proven: &mut BTreeSet<TypeId>,
) -> ActorMessageSafety {
    if proven.contains(&type_id) || !visiting.insert(type_id) {
        return ActorMessageSafety::Safe;
    }
    let Some(semantic) = resolver.arena().get(type_id).cloned() else {
        visiting.remove(&type_id);
        return unsafe_type(type_id, ActorMessageUnsafeKind::UnknownType);
    };
    let safety = match semantic {
        SemanticType::Primitive(_) | SemanticType::Enum { .. } => ActorMessageSafety::Safe,
        SemanticType::Tuple(elements) | SemanticType::Union(elements) => {
            actor_message_elements(resolver, elements, visiting, proven)
        }
        SemanticType::Record(fields) => actor_message_elements(
            resolver,
            fields
                .into_iter()
                .map(|(_, field_type)| field_type)
                .collect(),
            visiting,
            proven,
        ),
        SemanticType::TaggedUnion { .. } => {
            let Some(definition) = resolver.union_definition_for_type(type_id) else {
                visiting.remove(&type_id);
                return unsafe_type(type_id, ActorMessageUnsafeKind::UnknownType);
            };
            let elements = definition
                .cases()
                .iter()
                .flat_map(|case| {
                    case.parameters()
                        .iter()
                        .map(|(_, parameter_type, _)| *parameter_type)
                })
                .collect();
            actor_message_elements(resolver, elements, visiting, proven)
        }
        SemanticType::Optional(element) => {
            actor_message_safety(resolver, element, visiting, proven)
        }
        SemanticType::Array(_) | SemanticType::Table { .. } => {
            unsafe_type(type_id, ActorMessageUnsafeKind::MutableCollection)
        }
        SemanticType::Function { .. } => unsafe_type(type_id, ActorMessageUnsafeKind::Callable),
        SemanticType::Class { .. } => unsafe_type(type_id, ActorMessageUnsafeKind::Class),
        SemanticType::Interface { .. } => unsafe_type(type_id, ActorMessageUnsafeKind::Interface),
        SemanticType::Attribute { .. } => unsafe_type(type_id, ActorMessageUnsafeKind::Attribute),
        SemanticType::Builtin {
            definition: ACTOR_REF_TYPE_ID | ACTOR_REPLY_TYPE_ID,
            arguments,
        } => actor_message_elements(resolver, arguments, visiting, proven),
        SemanticType::Builtin { .. } => unsafe_type(type_id, ActorMessageUnsafeKind::Builtin),
        SemanticType::TypeParameter(_) => {
            unsafe_type(type_id, ActorMessageUnsafeKind::TypeParameter)
        }
        SemanticType::Opaque(_) => unsafe_type(type_id, ActorMessageUnsafeKind::Opaque),
        SemanticType::ErrorUnion { .. } => unsafe_type(type_id, ActorMessageUnsafeKind::ErrorUnion),
        SemanticType::Error => unsafe_type(type_id, ActorMessageUnsafeKind::CompilerError),
    };
    visiting.remove(&type_id);
    if safety == ActorMessageSafety::Safe {
        proven.insert(type_id);
    }
    safety
}

fn actor_message_elements(
    resolver: &SignatureResolver<'_>,
    elements: Vec<TypeId>,
    visiting: &mut BTreeSet<TypeId>,
    proven: &mut BTreeSet<TypeId>,
) -> ActorMessageSafety {
    for element in elements {
        let safety = actor_message_safety(resolver, element, visiting, proven);
        if safety != ActorMessageSafety::Safe {
            return safety;
        }
    }
    ActorMessageSafety::Safe
}

const fn unsafe_type(type_id: TypeId, kind: ActorMessageUnsafeKind) -> ActorMessageSafety {
    ActorMessageSafety::Unsafe { type_id, kind }
}
