// Copyright 2024 Golem Cloud
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::component::*;
use crate::AstCustomization;
use mappable_rc::Mrc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysedExport {
    Function(AnalysedFunction),
    Instance(AnalysedInstance),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysedFunction {
    pub name: String,
    pub params: Vec<AnalysedFunctionParameter>,
    pub results: AnalysedFunctionResults,
}

impl AnalysedFunction {
    pub fn is_constructor(&self) -> bool {
        self.name.starts_with("[constructor]")
            && self.results.len() == 1
            && matches!(
                &self.results.get_typ(0).unwrap(),
                AnalysedType::Resource {
                    resource_mode: AnalysedResourceMode::Owned,
                    ..
                }
            )
    }

    pub fn is_method(&self) -> bool {
        self.name.starts_with("[method]")
            && !self.params.is_empty()
            && matches!(
                &self.params[0].typ,
                AnalysedType::Resource {
                    resource_mode: AnalysedResourceMode::Borrowed,
                    ..
                }
            )
    }

    pub fn is_static_method(&self) -> bool {
        self.name.starts_with("[static]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysedInstance {
    pub name: String,
    pub funcs: Vec<AnalysedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysedType {
    Bool,
    S8,
    U8,
    S16,
    U16,
    S32,
    U32,
    S64,
    U64,
    F32,
    F64,
    Chr,
    Str,
    List(Box<AnalysedType>),
    Tuple(Vec<AnalysedType>),
    Record(Vec<(String, AnalysedType)>),
    Flags(Vec<String>),
    Enum(Vec<String>),
    Option(Box<AnalysedType>),
    Result {
        ok: Option<Box<AnalysedType>>,
        error: Option<Box<AnalysedType>>,
    },
    Variant(Vec<(String, Option<AnalysedType>)>),
    Resource {
        id: AnalysedResourceId,
        resource_mode: AnalysedResourceMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysedResourceMode {
    Owned,
    Borrowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysedResourceId {
    pub value: u64,
}

impl From<&PrimitiveValueType> for AnalysedType {
    fn from(value: &PrimitiveValueType) -> Self {
        match value {
            PrimitiveValueType::Bool => AnalysedType::Bool,
            PrimitiveValueType::S8 => AnalysedType::S8,
            PrimitiveValueType::U8 => AnalysedType::U8,
            PrimitiveValueType::S16 => AnalysedType::S16,
            PrimitiveValueType::U16 => AnalysedType::U16,
            PrimitiveValueType::S32 => AnalysedType::S32,
            PrimitiveValueType::U32 => AnalysedType::U32,
            PrimitiveValueType::S64 => AnalysedType::S64,
            PrimitiveValueType::U64 => AnalysedType::U64,
            PrimitiveValueType::F32 => AnalysedType::F32,
            PrimitiveValueType::F64 => AnalysedType::F64,
            PrimitiveValueType::Chr => AnalysedType::Chr,
            PrimitiveValueType::Str => AnalysedType::Str,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysedFunctionParameter {
    pub name: String,
    pub typ: AnalysedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysedFunctionResults {
    Named(Vec<NamedFunctionResult>),
    Unnamed(AnalysedType),
    Unit,
}

impl AnalysedFunctionResults {
    pub fn types(&self) -> Vec<&AnalysedType> {
        match self {
            AnalysedFunctionResults::Named(results) => results.iter().map(|r| &r.typ).collect(),
            AnalysedFunctionResults::Unnamed(result) => vec![result],
            AnalysedFunctionResults::Unit => vec![],
        }
    }

    pub fn names(&self) -> Vec<&str> {
        match self {
            AnalysedFunctionResults::Named(results) => {
                results.iter().map(|r| r.name.as_str()).collect()
            }
            AnalysedFunctionResults::Unnamed(_) => vec![],
            AnalysedFunctionResults::Unit => vec![],
        }
    }

    pub fn len(&self) -> usize {
        match self {
            AnalysedFunctionResults::Named(results) => results.len(),
            AnalysedFunctionResults::Unnamed(_) => 1,
            AnalysedFunctionResults::Unit => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_typ(&self, idx: usize) -> Option<&AnalysedType> {
        match self {
            AnalysedFunctionResults::Named(results) => results.get(idx).map(|x| &x.typ),
            AnalysedFunctionResults::Unnamed(typ) => {
                if idx == 0 {
                    Some(typ)
                } else {
                    None
                }
            }
            AnalysedFunctionResults::Unit => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedFunctionResult {
    pub name: String,
    pub typ: AnalysedType,
}

#[derive(Debug, Clone)]
pub enum AnalysisWarning {
    UnsupportedExport {
        kind: ComponentExternalKind,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub enum AnalysisFailure {
    Failed(String),
}

impl AnalysisFailure {
    pub fn failed(message: impl Into<String>) -> AnalysisFailure {
        AnalysisFailure::Failed(message.into())
    }

    pub fn fail_on_missing<T>(value: Option<T>, description: impl AsRef<str>) -> AnalysisResult<T> {
        match value {
            Some(value) => Ok(value),
            None => Err(AnalysisFailure::failed(format!(
                "Missing {}",
                description.as_ref()
            ))),
        }
    }
}

pub type AnalysisResult<A> = Result<A, AnalysisFailure>;

#[derive(Debug, Clone)]
struct ComponentStackItem<Ast: AstCustomization + 'static> {
    component: Mrc<Component<Ast>>,
    component_idx: Option<ComponentIdx>,
}

type ResourceIdMap = HashMap<(Vec<ComponentIdx>, ComponentTypeIdx), AnalysedResourceId>;

#[derive(Debug, Clone)]
pub struct AnalysisContext<Ast: AstCustomization + 'static> {
    component_stack: Vec<ComponentStackItem<Ast>>,
    warnings: Rc<RefCell<Vec<AnalysisWarning>>>,
    resource_ids: Rc<RefCell<ResourceIdMap>>,
}

impl<Ast: AstCustomization + 'static> AnalysisContext<Ast> {
    pub fn new(component: Component<Ast>) -> AnalysisContext<Ast> {
        AnalysisContext::from_rc(Mrc::new(component))
    }

    /// Initializes an analyzer for a given component
    pub fn from_rc(component: Mrc<Component<Ast>>) -> AnalysisContext<Ast> {
        AnalysisContext {
            component_stack: vec![ComponentStackItem {
                component,
                component_idx: None,
            }],
            warnings: Rc::new(RefCell::new(Vec::new())),
            resource_ids: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Get all top-level exports from the component with all the type information gathered from
    /// the component AST.
    pub fn get_top_level_exports(&self) -> AnalysisResult<Vec<AnalysedExport>> {
        let component = self.get_component();
        let mut result = Vec::new();
        for export in component.exports() {
            match export.kind {
                ComponentExternalKind::Func => {
                    let export = self.analyse_func_export(export.name.as_string(), export.idx)?;
                    result.push(AnalysedExport::Function(export));
                }
                ComponentExternalKind::Instance => {
                    let instance =
                        self.analyse_instance_export(export.name.as_string(), export.idx)?;
                    result.push(AnalysedExport::Instance(instance));
                }
                _ => self.warning(AnalysisWarning::UnsupportedExport {
                    kind: export.kind.clone(),
                    name: export.name.as_string(),
                }),
            }
        }

        Ok(result)
    }

    pub fn warnings(&self) -> Vec<AnalysisWarning> {
        self.warnings.borrow().clone()
    }

    fn warning(&self, warning: AnalysisWarning) {
        self.warnings.borrow_mut().push(warning);
    }

    fn get_resource_id(&self, type_idx: &ComponentTypeIdx) -> AnalysedResourceId {
        let new_unique_id = self.resource_ids.borrow().len() as u64;
        let mut resource_ids = self.resource_ids.borrow_mut();
        let path = self
            .component_stack
            .iter()
            .filter_map(|item| item.component_idx)
            .collect();
        let key = (path, *type_idx);
        resource_ids
            .entry(key)
            .or_insert_with(|| {
                AnalysedResourceId {
                    value: new_unique_id, // We don't to associate all IDs in each component, so this simple method can always generate a unique one
                }
            })
            .clone()
    }

    fn analyse_func_export(
        &self,
        name: String,
        idx: ComponentFuncIdx,
    ) -> AnalysisResult<AnalysedFunction> {
        let (function_section, next_ctx) = self
            .get_final_referenced(format!("component function {idx}"), |component| {
                component.get_component_func(idx)
            })?;
        let (func_type_section, next_ctx) = match &*function_section {
            ComponentSection::Canon(Canon::Lift { function_type, .. }) => next_ctx
                .get_final_referenced(
                    format!("component function type {function_type}"),
                    |component| component.get_component_type(*function_type),
                ),
            ComponentSection::Import(ComponentImport {
                desc: ComponentTypeRef::Func(func_type_idx),
                ..
            }) => next_ctx.get_final_referenced(
                format!("component function type {func_type_idx}"),
                |component| component.get_component_type(*func_type_idx),
            ),
            ComponentSection::Import(ComponentImport { desc, .. }) => Err(AnalysisFailure::failed(
                format!("Expected function import, but got {:?} instead", desc),
            )),
            _ => Err(AnalysisFailure::failed(format!(
                "Expected canonical lift function or function import, but got {} instead",
                function_section.type_name()
            ))),
        }?;
        match &*func_type_section {
            ComponentSection::Type(ComponentType::Func(func_type)) => {
                next_ctx.analyse_component_func_type(name, func_type)
            }
            _ => Err(AnalysisFailure::failed(format!(
                "Expected function type, but got {} instead",
                func_type_section.type_name()
            ))),
        }
    }

    fn analyse_component_func_type(
        &self,
        name: String,
        func_type: &ComponentFuncType,
    ) -> AnalysisResult<AnalysedFunction> {
        let mut params: Vec<AnalysedFunctionParameter> = Vec::new();
        for (param_name, param_type) in &func_type.params {
            params.push(AnalysedFunctionParameter {
                name: param_name.clone(),
                typ: self.analyse_component_val_type(param_type)?,
            })
        }

        let analysed_function_results = match &func_type.result {
            ComponentFuncResult::Unnamed(tpe) => {
                AnalysedFunctionResults::Unnamed(self.analyse_component_val_type(tpe)?)
            }

            ComponentFuncResult::Named(name_type_pairs) if name_type_pairs.is_empty() => {
                AnalysedFunctionResults::Unit
            }

            ComponentFuncResult::Named(name_type_pairs) => {
                let mut results: Vec<NamedFunctionResult> = Vec::new();

                for (result_name, result_type) in name_type_pairs {
                    results.push(NamedFunctionResult {
                        name: result_name.clone(),
                        typ: self.analyse_component_val_type(result_type)?,
                    })
                }

                AnalysedFunctionResults::Named(results)
            }
        };

        Ok(AnalysedFunction {
            name,
            params,
            results: analysed_function_results,
        })
    }

    fn analyse_component_type_idx(
        &self,
        component_type_idx: &ComponentTypeIdx,
        analysed_resource_mode: Option<AnalysedResourceMode>,
    ) -> AnalysisResult<AnalysedType> {
        let (component_type_section, next_ctx) = self.get_final_referenced(
            format!("component type {component_type_idx}"),
            |component| component.get_component_type(*component_type_idx),
        )?;
        match &*component_type_section {
            ComponentSection::Type(ComponentType::Defined(component_defined_type)) => {
                next_ctx.analyse_component_defined_type(component_defined_type)
            }
            ComponentSection::Type(ComponentType::Func(_)) => Err(AnalysisFailure::failed(
                "Passing functions in exported functions is not supported",
            )),
            ComponentSection::Type(ComponentType::Component(_)) => Err(AnalysisFailure::failed(
                "Passing components in exported functions is not supported",
            )),
            ComponentSection::Type(ComponentType::Instance(_)) => Err(AnalysisFailure::failed(
                "Passing instances in exported functions is not supported",
            )),
            ComponentSection::Type(ComponentType::Resource { .. }) => Err(AnalysisFailure::failed(
                "Passing resources in exported functions is not supported",
            )),
            ComponentSection::Import(ComponentImport { desc, .. }) => match desc {
                ComponentTypeRef::Type(TypeBounds::Eq(component_type_idx)) => {
                    self.analyse_component_type_idx(component_type_idx, analysed_resource_mode)
                }
                ComponentTypeRef::Type(TypeBounds::SubResource) => {
                    match analysed_resource_mode {
                        Some(resource_mode) => {
                            let id = next_ctx.get_resource_id(component_type_idx);
                            Ok(AnalysedType::Resource {
                                id,
                                resource_mode,
                            })
                        }
                        None => Err(AnalysisFailure::failed("Reached a sub-resource type bound without a surrounding borrowed/owned resource type")),
                    }
                }
                _ => Err(AnalysisFailure::failed(format!(
                    "Imports {desc:?} is not supported as a defined type"
                ))),
            },
            _ => Err(AnalysisFailure::failed(format!(
                "Expected component type, but got {} instead",
                component_type_section.type_name()
            ))),
        }
    }

    fn analyse_component_val_type(&self, tpe: &ComponentValType) -> AnalysisResult<AnalysedType> {
        match tpe {
            ComponentValType::Primitive(primitive_value_type) => Ok(primitive_value_type.into()),
            ComponentValType::Defined(component_type_idx) => {
                self.analyse_component_type_idx(component_type_idx, None)
            }
        }
    }

    fn analyse_component_defined_type(
        &self,
        defined_type: &ComponentDefinedType,
    ) -> AnalysisResult<AnalysedType> {
        match defined_type {
            ComponentDefinedType::Primitive { typ } => Ok(typ.into()),
            ComponentDefinedType::Record { fields } => {
                let mut result = Vec::new();
                for (name, typ) in fields {
                    result.push((name.clone(), self.analyse_component_val_type(typ)?));
                }
                Ok(AnalysedType::Record(result))
            }
            ComponentDefinedType::Variant { cases } => {
                let mut result = Vec::new();
                for case in cases {
                    result.push((
                        case.name.clone(),
                        case.typ
                            .as_ref()
                            .map(|t| self.analyse_component_val_type(t))
                            .transpose()?,
                    ));
                }
                Ok(AnalysedType::Variant(result))
            }
            ComponentDefinedType::List { elem } => Ok(AnalysedType::List(Box::new(
                self.analyse_component_val_type(elem)?,
            ))),
            ComponentDefinedType::Tuple { elems } => {
                let mut result = Vec::new();
                for elem in elems {
                    result.push(self.analyse_component_val_type(elem)?);
                }
                Ok(AnalysedType::Tuple(result))
            }
            ComponentDefinedType::Flags { names } => Ok(AnalysedType::Flags(names.clone())),
            ComponentDefinedType::Enum { names } => Ok(AnalysedType::Enum(names.clone())),
            ComponentDefinedType::Option { typ } => Ok(AnalysedType::Option(Box::new(
                self.analyse_component_val_type(typ)?,
            ))),
            ComponentDefinedType::Result { ok, err } => Ok(AnalysedType::Result {
                ok: ok
                    .as_ref()
                    .map(|t| self.analyse_component_val_type(t).map(Box::new))
                    .transpose()?,
                error: err
                    .as_ref()
                    .map(|t| self.analyse_component_val_type(t).map(Box::new))
                    .transpose()?,
            }),
            ComponentDefinedType::Owned { type_idx } => {
                self.analyse_component_type_idx(type_idx, Some(AnalysedResourceMode::Owned))
            }
            ComponentDefinedType::Borrowed { type_idx } => {
                self.analyse_component_type_idx(type_idx, Some(AnalysedResourceMode::Borrowed))
            }
        }
    }

    fn analyse_instance_export(
        &self,
        name: String,
        idx: InstanceIdx,
    ) -> AnalysisResult<AnalysedInstance> {
        let (instance_section, next_ctx) = self
            .get_final_referenced(format!("instance {idx}"), |component| {
                component.get_instance_wrapped(idx)
            })?;
        match &*instance_section {
            ComponentSection::Instance(instance) => match instance {
                ComponentInstance::Instantiate { component_idx, .. } => {
                    let (component_section, next_ctx) = next_ctx.get_final_referenced(
                        format!("component {component_idx}"),
                        |component| component.get_component(*component_idx),
                    )?;

                    match &*component_section {
                        ComponentSection::Component(referenced_component) => {
                            let next_ctx = next_ctx.push_component(
                                Mrc::map(component_section.clone(), |c| c.as_component()),
                                *component_idx,
                            );
                            let mut funcs = Vec::new();
                            for export in referenced_component.exports() {
                                match export.kind {
                                    ComponentExternalKind::Func => {
                                        let func = next_ctx.analyse_func_export(
                                            export.name.as_string(),
                                            export.idx,
                                        )?;
                                        funcs.push(func);
                                    }
                                    _ => next_ctx.warning(AnalysisWarning::UnsupportedExport {
                                        kind: export.kind.clone(),
                                        name: export.name.as_string(),
                                    }),
                                }
                            }

                            Ok(AnalysedInstance { name, funcs })
                        }
                        _ => Err(AnalysisFailure::failed(format!(
                            "Expected component, but got {} instead",
                            component_section.type_name()
                        ))),
                    }
                }
                ComponentInstance::FromExports { .. } => Err(AnalysisFailure::failed(
                    "Instance defined directly from exports are not supported",
                )),
            },
            _ => Err(AnalysisFailure::failed(format!(
                "Expected instance, but got {} instead",
                instance_section.type_name()
            ))),
        }
    }

    fn get_component(&self) -> Mrc<Component<Ast>> {
        self.component_stack.last().unwrap().component.clone()
    }

    fn get_components_from_stack(&self, count: u32) -> Vec<ComponentStackItem<Ast>> {
        self.component_stack
            .iter()
            .take(self.component_stack.len() - count as usize)
            .cloned()
            .collect()
    }

    fn push_component(
        &self,
        component: Mrc<Component<Ast>>,
        component_idx: ComponentIdx,
    ) -> AnalysisContext<Ast> {
        let mut component_stack = self.component_stack.clone();
        component_stack.push(ComponentStackItem {
            component,
            component_idx: Some(component_idx),
        });
        self.with_component_stack(component_stack)
    }

    fn with_component_stack(
        &self,
        component_stack: Vec<ComponentStackItem<Ast>>,
    ) -> AnalysisContext<Ast> {
        AnalysisContext {
            component_stack,
            warnings: self.warnings.clone(),
            resource_ids: self.resource_ids.clone(),
        }
    }

    fn get_final_referenced<F>(
        &self,
        description: impl AsRef<str>,
        f: F,
    ) -> AnalysisResult<(Mrc<ComponentSection<Ast>>, AnalysisContext<Ast>)>
    where
        F: Fn(&Component<Ast>) -> Option<Mrc<ComponentSection<Ast>>>,
    {
        let component = self.get_component();
        let direct_section = AnalysisFailure::fail_on_missing(f(&component), description)?;
        self.follow_redirects(direct_section)
    }

    fn follow_redirects(
        &self,
        section: Mrc<ComponentSection<Ast>>,
    ) -> AnalysisResult<(Mrc<ComponentSection<Ast>>, AnalysisContext<Ast>)> {
        let component = self.get_component();
        match &*section {
            ComponentSection::Export(ComponentExport { kind, idx, .. }) => {
                let next = match kind {
                    ComponentExternalKind::Module => AnalysisFailure::fail_on_missing(
                        component.get_module(*idx),
                        format!("module {idx}"),
                    )?,
                    ComponentExternalKind::Func => AnalysisFailure::fail_on_missing(
                        component.get_component_func(*idx),
                        format!("function {idx}"),
                    )?,
                    ComponentExternalKind::Value => AnalysisFailure::fail_on_missing(
                        component.get_value(*idx),
                        format!("value {idx}"),
                    )?,
                    ComponentExternalKind::Type => AnalysisFailure::fail_on_missing(
                        component.get_component_type(*idx),
                        format!("type {idx}"),
                    )?,
                    ComponentExternalKind::Instance => AnalysisFailure::fail_on_missing(
                        component.get_instance_wrapped(*idx),
                        format!("instance {idx}"),
                    )?,
                    ComponentExternalKind::Component => AnalysisFailure::fail_on_missing(
                        component.get_component(*idx),
                        format!("component {idx}"),
                    )?,
                };
                self.follow_redirects(next)
            }
            ComponentSection::Alias(Alias::InstanceExport {
                instance_idx, name, ..
            }) => {
                let (instance_section, next_ctx) = self
                    .get_final_referenced(format!("instance {instance_idx}"), |component| {
                        component.get_instance_wrapped(*instance_idx)
                    })?;
                match &*instance_section {
                    ComponentSection::Instance(_) => {
                        let instance = Mrc::map(instance_section, |s| s.as_instance());
                        let (maybe_export, next_ctx) =
                            next_ctx.find_export_by_name(&instance, name)?;
                        let export = AnalysisFailure::fail_on_missing(maybe_export, format!("missing aliased instance export {name} from instance {instance_idx}"))?;
                        let wrapped = Mrc::new(ComponentSection::Export((*export).clone()));
                        next_ctx.follow_redirects(wrapped)
                    }
                    _ => Err(AnalysisFailure::failed(format!(
                        "Expected instance, but got {} instead",
                        instance_section.type_name()
                    ))),
                }
            }
            ComponentSection::Import(ComponentImport {
                desc: ComponentTypeRef::Type(TypeBounds::Eq(idx)),
                ..
            }) => {
                let maybe_tpe = component.get_component_type(*idx);
                let tpe = AnalysisFailure::fail_on_missing(maybe_tpe, format!("type {idx}"))?;
                Ok((tpe, self.clone()))
            }
            ComponentSection::Alias(Alias::Outer {
                kind,
                target: AliasTarget { count, index },
            }) => {
                let referenced_components = self.get_components_from_stack(*count);
                let referenced_component = referenced_components
                    .first()
                    .ok_or(AnalysisFailure::failed(format!(
                        "Component stack underflow (count={count}, size={}",
                        self.component_stack.len()
                    )))?
                    .component
                    .clone();
                match kind {
                    OuterAliasKind::CoreModule => Err(AnalysisFailure::failed(
                        "Core module aliases are not supported",
                    )),
                    OuterAliasKind::CoreType => Err(AnalysisFailure::failed(
                        "Core type aliases are not supported",
                    )),
                    OuterAliasKind::Type => {
                        let maybe_tpe = referenced_component.get_component_type(*index);
                        let tpe =
                            AnalysisFailure::fail_on_missing(maybe_tpe, format!("type {index}"))?;
                        let next_ctx = self.with_component_stack(referenced_components);
                        next_ctx.follow_redirects(tpe)
                    }
                    OuterAliasKind::Component => {
                        let maybe_component = referenced_component.get_component(*index);
                        let component = AnalysisFailure::fail_on_missing(
                            maybe_component,
                            format!("component {index}"),
                        )?;
                        let next_ctx = self.with_component_stack(referenced_components);
                        next_ctx.follow_redirects(component)
                    }
                }
            }
            // TODO: support other redirections if needed
            _ => Ok((section, self.clone())),
        }
    }

    fn find_export_by_name(
        &self,
        instance: &ComponentInstance,
        name: &String,
    ) -> AnalysisResult<(Option<Mrc<ComponentExport>>, AnalysisContext<Ast>)> {
        match instance {
            ComponentInstance::Instantiate { component_idx, .. } => {
                let (component, next_ctx) = self
                    .get_final_referenced(format!("component {component_idx}"), |component| {
                        component.get_component(*component_idx)
                    })?;
                let component = Mrc::map(component, |c| c.as_component());
                let export = component
                    .exports()
                    .iter()
                    .find(|export| export.name == *name)
                    .cloned();
                Ok((export, next_ctx.push_component(component, *component_idx)))
            }
            ComponentInstance::FromExports { exports } => {
                let export = exports.iter().find(|export| export.name == *name).cloned();
                Ok((export.map(Mrc::new), self.clone()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::{
        AnalysedFunction, AnalysedFunctionParameter, AnalysedFunctionResults, AnalysedResourceId,
        AnalysedResourceMode, AnalysedType,
    };

    #[test]
    fn analysed_function_kind() {
        let cons = AnalysedFunction {
            name: "[constructor]cart".to_string(),
            params: vec![AnalysedFunctionParameter {
                name: "user-id".to_string(),
                typ: AnalysedType::Str,
            }],
            results: AnalysedFunctionResults::Unnamed(AnalysedType::Resource {
                id: AnalysedResourceId { value: 0 },
                resource_mode: AnalysedResourceMode::Owned,
            }),
        };

        let method = AnalysedFunction {
            name: "[method]cart.add-item".to_string(),
            params: vec![
                AnalysedFunctionParameter {
                    name: "self".to_string(),
                    typ: AnalysedType::Resource {
                        id: AnalysedResourceId { value: 0 },
                        resource_mode: AnalysedResourceMode::Borrowed,
                    },
                },
                AnalysedFunctionParameter {
                    name: "item".to_string(),
                    typ: AnalysedType::Record(vec![
                        ("product-id".to_string(), AnalysedType::Str),
                        ("name".to_string(), AnalysedType::Str),
                        ("price".to_string(), AnalysedType::F32),
                        ("quantity".to_string(), AnalysedType::U32),
                    ]),
                },
            ],
            results: AnalysedFunctionResults::Unit,
        };
        let static_method = AnalysedFunction {
            name: "[static]cart.merge".to_string(),
            params: vec![
                AnalysedFunctionParameter {
                    name: "self".to_string(),
                    typ: AnalysedType::Resource {
                        id: AnalysedResourceId { value: 0 },
                        resource_mode: AnalysedResourceMode::Borrowed,
                    },
                },
                AnalysedFunctionParameter {
                    name: "that".to_string(),
                    typ: AnalysedType::Resource {
                        id: AnalysedResourceId { value: 0 },
                        resource_mode: AnalysedResourceMode::Borrowed,
                    },
                },
            ],
            results: AnalysedFunctionResults::Unnamed(AnalysedType::Resource {
                id: AnalysedResourceId { value: 0 },
                resource_mode: AnalysedResourceMode::Owned,
            }),
        };
        let fun = AnalysedFunction {
            name: "hash".to_string(),
            params: vec![AnalysedFunctionParameter {
                name: "path".to_string(),
                typ: AnalysedType::Str,
            }],
            results: AnalysedFunctionResults::Unnamed(AnalysedType::Result {
                ok: Some(Box::new(AnalysedType::Record(vec![
                    ("lower".to_string(), AnalysedType::U64),
                    ("upper".to_string(), AnalysedType::U64),
                ]))),
                error: Some(Box::new(AnalysedType::Str)),
            }),
        };

        assert!(cons.is_constructor());
        assert!(method.is_method());
        assert!(static_method.is_static_method());
        assert!(!fun.is_constructor());
        assert!(!fun.is_method());
        assert!(!fun.is_static_method());
    }
}
