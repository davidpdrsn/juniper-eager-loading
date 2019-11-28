mod field_args;

use field_args::{
    EagerLoading, FieldArgs, HasMany, HasManyThrough, HasOne, OptionHasOne, RootModelField, Spanned,
};
use heck::{CamelCase, SnakeCase};
use proc_macro2::{Span, TokenStream};
use proc_macro_error::*;
use quote::{format_ident, quote};
use syn::spanned::Spanned as _;
use syn::{parse_macro_input, Fields, GenericArgument, Ident, ItemStruct, PathArguments, Type};

pub fn gen_tokens(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item_struct = parse_macro_input!(tokens as ItemStruct);

    let ItemStruct {
        ident: struct_name,
        attrs,
        fields,
        ..
    } = item_struct;

    let args = match EagerLoading::from_attributes(&attrs) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };

    let out = DeriveData {
        struct_name,
        args,
        fields,
        out: TokenStream::new(),
    };

    out.build_derive_output().into()
}

struct DeriveData {
    struct_name: Ident,
    fields: Fields,
    args: EagerLoading,
    out: TokenStream,
}

impl DeriveData {
    fn build_derive_output(mut self) -> TokenStream {
        self.gen_graphql_node_for_model();
        self.gen_eager_load_all_children();

        self.gen_eager_load_children_of_type();

        if self.args.print() {
            eprintln!("{}", self.out);
        }

        self.out
    }

    fn gen_graphql_node_for_model(&mut self) {
        let struct_name = self.struct_name();
        let model = self.model();
        let id = self.id();
        let context = self.context();
        let error = self.error();

        let field_setters = self.struct_fields().map(|field| {
            let ident = &field.ident;

            if is_association_field(&field.ty) {
                quote! { #ident: std::default::Default::default() }
            } else {
                quote! { #ident: std::clone::Clone::clone(model) }
            }
        });

        let code = quote! {
            impl juniper_eager_loading::GraphqlNodeForModel for #struct_name {
                type Model = #model;
                type Id = #id;
                type Context = #context;
                type Error = #error;

                fn new_from_model(model: &Self::Model) -> Self {
                    Self {
                        #(#field_setters),*
                    }
                }
            }
        };
        self.out.extend(code);
    }

    fn gen_eager_load_children_of_type(&mut self) {
        let impls = self
            .struct_fields()
            .filter_map(|field| self.gen_eager_load_children_of_type_for_field(field));

        let code = quote! { #(#impls)* };
        self.out.extend(code);
    }

    fn gen_eager_load_children_of_type_for_field(&self, field: &syn::Field) -> Option<TokenStream> {
        let data = self.parse_field_args(field)?;

        if data.args.skip() {
            return Some(quote! {});
        }

        let inner_type = &data.inner_type;
        let struct_name = self.struct_name();
        let join_model_impl = self.join_model_impl(&data);
        let load_children_impl = self.load_children_impl(&data);
        let association_impl = self.association_impl(&data);
        let is_child_of_impl = self.is_child_of_impl(&data);
        let context = self.field_impl_context_name(&field);
        let field_arguments = data.args.field_arguments();

        let full_output = quote! {
            #[allow(missing_docs, dead_code)]
            struct #context;

            impl<'a> juniper_eager_loading::EagerLoadChildrenOfType<
                'a,
                #inner_type,
                #context,
                #join_model_impl,
            > for #struct_name {
                type FieldArguments = #field_arguments;

                #load_children_impl
                #is_child_of_impl
                #association_impl
            }
        };

        if data.args.print() {
            eprintln!("{}", full_output);
        }

        Some(full_output)
    }

    fn parse_field_args(&self, field: &syn::Field) -> Option<FieldDeriveData> {
        let inner_type = get_type_from_association(&field.ty)?.clone();
        let association_type = association_type(&field.ty)?;
        let span = field.span();

        let args = match association_type {
            AssociationType::HasOne => {
                let args = HasOne::from_attributes(&field.attrs)
                    .unwrap_or_else(|e| abort!(e.span(), "{}", e));
                FieldArgs::HasOne(Spanned::new(span, args))
            }
            AssociationType::OptionHasOne => {
                let args = OptionHasOne::from_attributes(&field.attrs)
                    .unwrap_or_else(|e| abort!(e.span(), "{}", e));
                FieldArgs::OptionHasOne(Spanned::new(span, args))
            }
            AssociationType::HasMany => {
                let args = HasMany::from_attributes(&field.attrs)
                    .unwrap_or_else(|e| abort!(e.span(), "{}", e));
                FieldArgs::HasMany(Spanned::new(span, args))
            }
            AssociationType::HasManyThrough => {
                let args = HasManyThrough::from_attributes(&field.attrs)
                    .unwrap_or_else(|e| abort!(e.span(), "{}", e));
                FieldArgs::HasManyThrough(Spanned::new(span, Box::new(args)))
            }
        };

        let field_name = field
            .ident
            .as_ref()
            .cloned()
            .unwrap_or_else(|| abort!(span, "Found association field without a name"));

        let foreign_key_field_default = match args {
            FieldArgs::HasOne(_) | FieldArgs::OptionHasOne(_) => &field_name,
            FieldArgs::HasMany(_) | FieldArgs::HasManyThrough(_) => self.struct_name(),
        }
        .clone();

        let data = FieldDeriveData {
            field_name,
            inner_type,
            foreign_key_field_default,
            args,
        };

        Some(data)
    }

    fn join_model_impl(&self, data: &FieldDeriveData) -> TokenStream {
        match &data.args {
            FieldArgs::HasMany(_) | FieldArgs::HasOne(_) | FieldArgs::OptionHasOne(_) => {
                quote! { () }
            }
            FieldArgs::HasManyThrough(has_many_through) => {
                let join_model = has_many_through.join_model(has_many_through.span());
                quote! { #join_model }
            }
        }
    }

    fn load_children_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let join_model: syn::Type;
        let foreign_key_field = &data.args.foreign_key_field(&data.foreign_key_field_default);
        let inner_type = &data.inner_type;

        let load_children_impl = match &data.args {
            FieldArgs::HasOne(_) => {
                join_model = syn::parse_str::<syn::Type>("()").unwrap();

                quote! {
                    let ids = models
                        .iter()
                        .map(|model| model.#foreign_key_field.clone())
                        .collect::<Vec<_>>();
                    let ids = juniper_eager_loading::unique(ids);

                    let child_models: Vec<<#inner_type as juniper_eager_loading::GraphqlNodeForModel>::Model> =
                        juniper_eager_loading::LoadFrom::load(&ids, field_args, ctx)?;

                    Ok(juniper_eager_loading::LoadChildrenOutput::ChildModels(child_models))
                }
            }
            FieldArgs::OptionHasOne(_) => {
                join_model = syn::parse_str::<syn::Type>("()").unwrap();

                quote! {
                    let ids = models
                        .iter()
                        .filter_map(|model| model.#foreign_key_field)
                        .map(|id| id.clone())
                        .collect::<Vec<_>>();
                    let ids = juniper_eager_loading::unique(ids);

                    let child_models: Vec<<#inner_type as juniper_eager_loading::GraphqlNodeForModel>::Model> =
                        juniper_eager_loading::LoadFrom::load(&ids, field_args, ctx)?;

                    Ok(juniper_eager_loading::LoadChildrenOutput::ChildModels(child_models))
                }
            }
            FieldArgs::HasMany(has_many) => {
                join_model = syn::parse_str::<syn::Type>("()").unwrap();

                let filter = if let Some(predicate_method) = has_many.predicate_method() {
                    quote! {
                        let child_models = child_models
                            .into_iter()
                            .filter(|child_model| child_model.#predicate_method(ctx))
                            .collect::<Vec<_>>();
                    }
                } else {
                    quote! {}
                };

                quote! {
                    let child_models: Vec<<#inner_type as juniper_eager_loading::GraphqlNodeForModel>::Model> =
                        juniper_eager_loading::LoadFrom::load(&models, field_args, ctx)?;

                    #filter

                    Ok(juniper_eager_loading::LoadChildrenOutput::ChildModels(child_models))
                }
            }
            FieldArgs::HasManyThrough(has_many_through) => {
                join_model = has_many_through.join_model(has_many_through.span());

                let model_id_field = has_many_through.model_id_field(&data.inner_type);

                let filter = if let Some(predicate_method) = has_many_through.predicate_method() {
                    quote! {
                        let join_models = join_models
                            .into_iter()
                            .filter(|child_model| child_model.#predicate_method(ctx))
                            .collect::<Vec<_>>();
                    }
                } else {
                    quote! {}
                };

                quote! {
                    let join_models: Vec<#join_model> =
                        juniper_eager_loading::LoadFrom::load(&models, field_args, ctx)?;

                    #filter

                    let child_models: Vec<<#inner_type as juniper_eager_loading::GraphqlNodeForModel>::Model> =
                        juniper_eager_loading::LoadFrom::load(&join_models, field_args, ctx)?;

                    let mut child_and_join_model_pairs = Vec::new();
                    for join_model in join_models {
                        for child_model in &child_models {
                            if join_model.#model_id_field == child_model.id {
                                let pair = (
                                    std::clone::Clone::clone(child_model),
                                    std::clone::Clone::clone(&join_model),
                                );
                                child_and_join_model_pairs.push(pair);
                            }
                        }
                    }

                    Ok(juniper_eager_loading::LoadChildrenOutput::ChildAndJoinModels(
                        child_and_join_model_pairs
                    ))
                }
            }
        };

        quote! {
            #[allow(unused_variables)]
            fn load_children(
                models: &[Self::Model],
                field_args: &Self::FieldArguments,
                ctx: &Self::Context,
            ) -> Result<
                juniper_eager_loading::LoadChildrenOutput<
                    <#inner_type as juniper_eager_loading::GraphqlNodeForModel>::Model,
                    #join_model
                >,
                Self::Error,
            > {
                #load_children_impl
            }
        }
    }

    fn is_child_of_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let root_model_field = self.root_model_field();
        let foreign_key_field = &data.args.foreign_key_field(&data.foreign_key_field_default);
        let inner_type = &data.inner_type;
        let mut join_model = syn::parse_str::<syn::Type>("()").unwrap();
        let field_name = &data.field_name;

        let is_child_of_impl = match &data.args {
            FieldArgs::HasOne(has_one) => {
                let field_root_model_field = has_one.root_model_field(field_name);

                quote! {
                    node.#root_model_field.#foreign_key_field == child.#field_root_model_field.id
                }
            }
            FieldArgs::OptionHasOne(option_has_one) => {
                let field_root_model_field = option_has_one.root_model_field(field_name);

                quote! {
                    node.#root_model_field.#foreign_key_field == Some(child.#field_root_model_field.id)
                }
            }
            FieldArgs::HasMany(has_many) => {
                let field_root_model_field = has_many.root_model_field(field_name);

                if has_many.foreign_key_optional.is_some() {
                    quote! {
                        Some(node.#root_model_field.id) ==
                            child.#field_root_model_field.#foreign_key_field
                    }
                } else {
                    quote! {
                        node.#root_model_field.id ==
                            child.#field_root_model_field.#foreign_key_field
                    }
                }
            }
            FieldArgs::HasManyThrough(has_many_through) => {
                join_model = has_many_through.join_model(has_many_through.span());
                let model_field = has_many_through.model_field(&data.inner_type);
                let model_id_field = has_many_through.model_id_field(&data.inner_type);

                quote! {
                    node.#root_model_field.id == join_model.#foreign_key_field &&
                        join_model.#model_id_field == child.#model_field.id
                }
            }
        };

        quote! {
            fn is_child_of(
                node: &Self,
                child: &#inner_type,
                join_model: &#join_model,
                _field_args: &Self::FieldArguments,
                context: &Self::Context,
            ) -> bool {
                #is_child_of_impl
            }
        }
    }

    fn association_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let field_name = &data.field_name;
        let inner_type = &data.inner_type;

        quote! {
            fn association(node: &mut Self) ->
                &mut dyn juniper_eager_loading::Association<#inner_type>
            {
                &mut node.#field_name
            }
        }
    }

    fn gen_eager_load_all_children(&mut self) {
        let struct_name = self.struct_name();

        let eager_load_children_calls = self
            .struct_fields()
            .filter_map(|field| self.gen_eager_load_all_children_for_field(field));

        let code = quote! {
            impl juniper_eager_loading::EagerLoadAllChildren for #struct_name {
                fn eager_load_all_children_for_each(
                    nodes: &mut [Self],
                    models: &[Self::Model],
                    ctx: &Self::Context,
                    trail: &juniper_from_schema::QueryTrail<'_, Self, juniper_from_schema::Walked>,
                ) -> Result<(), Self::Error> {
                    #(#eager_load_children_calls)*

                    Ok(())
                }
            }
        };
        self.out.extend(code);
    }

    fn gen_eager_load_all_children_for_field(&self, field: &syn::Field) -> Option<TokenStream> {
        let inner_type = get_type_from_association(&field.ty)?;

        let data = self.parse_field_args(field)?;
        let args = data.args;

        let field_name = args
            .graphql_field()
            .clone()
            .map(|ident| {
                let ident = ident.to_string().to_snake_case();
                Ident::new(&ident, Span::call_site())
            })
            .unwrap_or_else(|| {
                field.ident.clone().unwrap_or_else(|| {
                    abort!(field.span(), "Found association field without a name")
                })
            });
        let field_args_name = format_ident!("{}_args", field_name);

        let impl_context = self.field_impl_context_name(&field);

        Some(quote! {
            if let Some(child_trail) = trail.#field_name().walk() {
                let field_args = trail.#field_args_name();

                EagerLoadChildrenOfType::<#inner_type, #impl_context, _>::eager_load_children(
                    nodes,
                    models,
                    &ctx,
                    &child_trail,
                    &field_args,
                )?;
            }
        })
    }

    fn struct_name(&self) -> &syn::Ident {
        &self.struct_name
    }

    fn model(&self) -> TokenStream {
        self.args.model(&self.struct_name())
    }

    fn id(&self) -> TokenStream {
        self.args.id()
    }

    fn context(&self) -> TokenStream {
        self.args.context()
    }

    fn error(&self) -> TokenStream {
        self.args.error()
    }

    fn root_model_field(&self) -> TokenStream {
        self.args.root_model_field(&self.struct_name())
    }

    fn struct_fields(&self) -> syn::punctuated::Iter<syn::Field> {
        self.fields.iter()
    }

    fn field_impl_context_name(&self, field: &syn::Field) -> Ident {
        let camel_name = field
            .ident
            .as_ref()
            .expect("field without name")
            .to_string()
            .to_camel_case();
        let full_name = format!("EagerLoadingContext{}For{}", self.struct_name(), camel_name);
        Ident::new(&full_name, Span::call_site())
    }
}

macro_rules! if_let_or_none {
    ( $path:path , $($tokens:tt)* ) => {
        if let $path(inner) = $($tokens)* {
            inner
        } else {
            return None
        }
    };
}

fn get_type_from_association(ty: &syn::Type) -> Option<&syn::Type> {
    if !is_association_field(ty) {
        return None;
    }

    let type_path = if_let_or_none!(Type::Path, ty);
    let path = &type_path.path;
    let segments = &path.segments;
    let segment = if_let_or_none!(Some, segments.last());
    let args = if_let_or_none!(PathArguments::AngleBracketed, &segment.arguments);
    let generic_argument: &syn::GenericArgument = if_let_or_none!(Some, args.args.last());
    let ty = if_let_or_none!(GenericArgument::Type, generic_argument);
    Some(remove_possible_box_wrapper(ty))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum AssociationType {
    HasOne,
    OptionHasOne,
    HasMany,
    HasManyThrough,
}

fn association_type(ty: &syn::Type) -> Option<AssociationType> {
    if *last_ident_in_type_segment(ty)? == "OptionHasOne" {
        return Some(AssociationType::OptionHasOne);
    }

    if *last_ident_in_type_segment(ty)? == "HasManyThrough" {
        return Some(AssociationType::HasManyThrough);
    }

    if *last_ident_in_type_segment(ty)? == "HasMany" {
        return Some(AssociationType::HasMany);
    }

    if *last_ident_in_type_segment(ty)? == "HasOne" {
        return Some(AssociationType::HasOne);
    }

    None
}

fn is_association_field(ty: &syn::Type) -> bool {
    association_type(ty).is_some()
}

fn last_ident_in_type_segment(ty: &syn::Type) -> Option<&syn::Ident> {
    let type_path = if_let_or_none!(Type::Path, ty);
    let path = &type_path.path;
    let segments = &path.segments;
    let segment = if_let_or_none!(Some, segments.last());
    Some(&segment.ident)
}

#[derive(Debug)]
struct FieldDeriveData {
    field_name: Ident,
    inner_type: syn::Type,
    args: FieldArgs,
    foreign_key_field_default: Ident,
}

fn remove_possible_box_wrapper(ty: &Type) -> &syn::Type {
    if let Type::Path(type_path) = ty {
        let last_segment = if let Some(x) = type_path.path.segments.last() {
            x
        } else {
            return ty;
        };

        if last_segment.ident == "Box" {
            let args = if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                args
            } else {
                return ty;
            };

            let generic_argument = if let Some(x) = args.args.last() {
                x
            } else {
                return ty;
            };

            if let syn::GenericArgument::Type(inner_ty) = generic_argument {
                inner_ty
            } else {
                ty
            }
        } else {
            ty
        }
    } else {
        ty
    }
}
