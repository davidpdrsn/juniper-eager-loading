mod field_args;

use darling::{FromDeriveInput, FromMeta};
use field_args::{DeriveArgs, FieldArgs, HasMany, HasManyThrough, HasOne, OptionHasOne};
use heck::{CamelCase, SnakeCase};
use lazy_static::lazy_static;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::sync::atomic::{AtomicBool, Ordering};
use syn::{
    parse_macro_input, DeriveInput, GenericArgument, Ident, NestedMeta, PathArguments, Type,
};

pub fn gen_tokens(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let args = match DeriveArgs::from_derive_input(&ast) {
        Ok(args) => args,
        Err(err) => panic!("{}", err),
    };

    let out = DeriveData::new(ast, args);
    let tokens = out.build_derive_output();

    derive_macro_called();

    tokens.into()
}

lazy_static! {
    static ref FIRST_DERIVE_CALL: AtomicBool = { AtomicBool::new(true) };
}

fn derive_macro_called() {
    FIRST_DERIVE_CALL.store(false, Ordering::SeqCst)
}

fn first_time_calling_derive_macro() -> bool {
    FIRST_DERIVE_CALL.load(Ordering::SeqCst)
}

struct DeriveData {
    input: DeriveInput,
    args: DeriveArgs,
    tokens: TokenStream,
}

impl DeriveData {
    fn new(input: DeriveInput, args: DeriveArgs) -> Self {
        Self {
            input,
            args,
            tokens: quote! {},
        }
    }

    fn build_derive_output(mut self) -> TokenStream {
        if first_time_calling_derive_macro() {
            self.gen_impl_of_marker_trait();
        }

        self.gen_graphql_node_for_model();
        self.gen_eager_load_children_of_type();
        self.gen_eager_load_all_children();

        self.tokens
    }

    fn gen_impl_of_marker_trait(&mut self) {
        self.tokens.extend(quote! {
            impl<'a, T> juniper_eager_loading::GenericQueryTrail<T, juniper_from_schema::Walked>
                for QueryTrail<'a, T, juniper_from_schema::Walked>
            {
            }
        });
    }

    fn gen_graphql_node_for_model(&mut self) {
        let struct_name = self.struct_name();
        let model = self.model();
        let id = self.id();
        let connection = self.connection();
        let error = self.error();

        let field_setters = self.struct_fields().map(|field| {
            let ident = &field.ident;

            if is_association_field(&field.ty) {
                quote! { #ident: Default::default() }
            } else {
                quote! { #ident: std::clone::Clone::clone(model) }
            }
        });

        self.tokens.extend(quote! {
            impl juniper_eager_loading::GraphqlNodeForModel for #struct_name {
                type Model = #model;
                type Id = #id;
                type Connection = #connection;
                type Error = #error;

                fn new_from_model(model: &Self::Model) -> Self {
                    Self {
                        #(#field_setters),*
                    }
                }
            }
        });
    }

    fn gen_eager_load_children_of_type(&mut self) {
        let impls = self
            .struct_fields()
            .filter_map(|field| self.gen_eager_load_children_of_type_for_field(field));

        self.tokens.extend(quote! { #(#impls)* });
    }

    fn gen_eager_load_children_of_type_for_field(&self, field: &syn::Field) -> Option<TokenStream> {
        let (args, data) = self.parse_field_args(field)?;

        let inner_type = &data.inner_type;
        let struct_name = self.struct_name();
        let child_model = self.child_model_impl(&args, &data);
        let join_model_impl = self.join_model_impl(&data);
        let child_id = self.child_id(&data);
        let child_ids_impl = self.child_ids_impl(&data);
        let load_children_impl = self.load_children_impl(&data);
        let is_child_of_impl = self.is_child_of_impl(&data);
        let loaded_or_failed_child_impl = self.loaded_or_failed_child_impl(&data);
        let assert_loaded_otherwise_failed_impl = self.assert_loaded_otherwise_failed_impl(&data);

        let context = self.field_context_name(&field);

        let full_output = quote! {
            #[allow(missing_doc, dead_code)]
            struct #context;

            impl<'a> EagerLoadChildrenOfType<
                #inner_type,
                QueryTrail<'a, #inner_type, juniper_from_schema::Walked>,
                #context,
                #join_model_impl,
            > for #struct_name {
                type ChildModel = #child_model;
                type ChildId = #child_id;

                #child_ids_impl
                #load_children_impl
                #is_child_of_impl
                #loaded_or_failed_child_impl
                #assert_loaded_otherwise_failed_impl
            }
        };

        if args.print {
            println!("{}", full_output);
        }

        if args.skip {
            Some(quote! {})
        } else {
            Some(full_output)
        }
    }

    fn parse_field_args(&self, field: &syn::Field) -> Option<(FieldArgs, FieldDeriveData)> {
        let inner_type = get_type_from_association(&field.ty)?;
        let association_type = association_type(&field.ty)?;

        let args = match association_type {
            AssociationType::HasOne => {
                let args = parse_field_args::<HasOne>(&field)
                    .unwrap_or_else(|e| panic!("{}", e))
                    .has_one;
                FieldArgs::from(args)
            }
            AssociationType::OptionHasOne => {
                let args = parse_field_args::<OptionHasOne>(&field)
                    .unwrap_or_else(|e| panic!("{}", e))
                    .option_has_one;
                FieldArgs::from(args)
            }
            AssociationType::HasMany => {
                let args = parse_field_args::<HasMany>(&field)
                    .unwrap_or_else(|e| panic!("{}", e))
                    .has_many;
                FieldArgs::from(args)
            }
            AssociationType::HasManyThrough => {
                let args = parse_field_args::<HasManyThrough>(&field)
                    .unwrap_or_else(|e| panic!("{}", e))
                    .has_many_through;
                FieldArgs::from(args)
            }
        };

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::HasOne` field without a name")
        });

        let foreign_key_field_default = match association_type {
            AssociationType::HasMany | AssociationType::HasManyThrough => self.struct_name(),
            AssociationType::HasOne | AssociationType::OptionHasOne => &field_name,
        };

        let data = FieldDeriveData {
            field_name: field_name.clone(),
            inner_type: inner_type.clone(),
            root_model_field: self.root_model_field().clone(),
            join_model: args.join_model(),
            model_field: args.model_field(&inner_type),
            join_model_field: args.join_model_field(),
            foreign_key_field: args.foreign_key_field(foreign_key_field_default),
            foreign_key_optional: args.foreign_key_optional,
            field_root_model_field: args.root_model_field(&field_name),
            association_type,
            predicate_method: args.predicate_method(),
        };

        Some((args, data))
    }

    fn join_model_impl(&self, data: &FieldDeriveData) -> TokenStream {
        match data.association_type {
            AssociationType::HasMany | AssociationType::HasOne | AssociationType::OptionHasOne => {
                quote! { () }
            }
            AssociationType::HasManyThrough => {
                let join_model = &data.join_model;
                quote! { #join_model }
            }
        }
    }

    fn child_model_impl(&self, args: &FieldArgs, data: &FieldDeriveData) -> TokenStream {
        let tokens = args.model(&data.inner_type);

        match data.association_type {
            AssociationType::HasMany
            | AssociationType::HasOne
            | AssociationType::OptionHasOne
            | AssociationType::HasManyThrough => {
                quote! { #tokens }
            }
        }
    }

    fn child_ids_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let foreign_key_field = &data.foreign_key_field;
        let join_model = &data.join_model;
        let model_id_field = data.model_id_field();

        let child_ids_impl = match data.association_type {
            AssociationType::HasOne | AssociationType::OptionHasOne => {
                quote! {
                    let ids = models
                        .iter()
                        .map(|model| model.#foreign_key_field.clone())
                        .collect::<Vec<_>>();
                    let ids = juniper_eager_loading::unique(ids);
                    Ok(juniper_eager_loading::LoadResult::Ids(ids))
                }
            }
            AssociationType::HasMany => {
                let filter = if let Some(predicate_method) = &data.predicate_method {
                    quote! {
                        let child_models = child_models
                            .into_iter()
                            .filter(|child_model| child_model.#predicate_method(db))
                            .collect::<Vec<_>>();
                    }
                } else {
                    quote! {}
                };

                quote! {
                    let child_models = <
                        Self::ChildModel
                        as
                        juniper_eager_loading::LoadFrom<Self::Model>
                    >::load(&models, db)?;

                    #filter

                    let child_models = child_models
                        .into_iter()
                        .map(|child| (child, ()))
                        .collect();

                    Ok(juniper_eager_loading::LoadResult::Models(child_models))
                }
            }
            AssociationType::HasManyThrough => {
                let filter = if let Some(predicate_method) = &data.predicate_method {
                    quote! {
                        let join_models = join_models
                            .into_iter()
                            .filter(|child_model| child_model.#predicate_method(db))
                            .collect::<Vec<_>>();
                    }
                } else {
                    quote! {}
                };

                quote! {
                    let join_models = <
                        #join_model
                        as
                        juniper_eager_loading::LoadFrom<Self::Model>
                    >::load(&models, db)?;

                    #filter

                    let child_models = <
                        Self::ChildModel
                        as
                        juniper_eager_loading::LoadFrom<#join_model>
                    >::load(&join_models, db)?;

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

                    Ok(juniper_eager_loading::LoadResult::Models(child_and_join_model_pairs))
                }
            }
        };

        quote! {
            #[allow(unused_variables)]
            fn child_ids(
                models: &[Self::Model],
                db: &Self::Connection,
            ) -> Result<
                juniper_eager_loading::LoadResult<Self::ChildId, (Self::ChildModel, #join_model)>,
                Self::Error,
            > {
                #child_ids_impl
            }
        }
    }

    fn load_children_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let normalize_ids = self.normalize_ids(data);

        quote! {
            fn load_children(
                ids: &[Self::ChildId],
                db: &Self::Connection,
            ) -> Result<Vec<Self::ChildModel>, Self::Error> {
                #normalize_ids
                <
                    Self::ChildModel
                    as
                    juniper_eager_loading::LoadFrom<Self::Id>
                >::load(&ids, db)
            }
        }
    }

    fn normalize_ids(&self, data: &FieldDeriveData) -> TokenStream {
        match data.association_type {
            AssociationType::HasOne => {
                quote! {}
            }
            AssociationType::OptionHasOne => {
                quote! {
                    let ids = ids
                        .into_iter()
                        .filter_map(|id| id .as_ref())
                        .cloned()
                        .collect::<Vec<_>>();
                    let ids = juniper_eager_loading::unique(ids);
                }
            }
            AssociationType::HasMany | AssociationType::HasManyThrough => {
                quote! {
                    let ids = ids.iter().flatten().cloned().collect::<Vec<_>>();
                    let ids = juniper_eager_loading::unique(ids);
                }
            }
        }
    }

    fn is_child_of_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let root_model_field = &data.root_model_field;
        let foreign_key_field = &data.foreign_key_field;
        let field_root_model_field = &data.field_root_model_field;
        let inner_type = &data.inner_type;
        let join_model = &data.join_model;
        let model_field = &data.model_field;
        let model_id_field = &data.model_id_field();

        let is_child_of_impl = match data.association_type {
            AssociationType::HasOne => {
                quote! {
                    node.#root_model_field.#foreign_key_field == (child.0).#field_root_model_field.id
                }
            }
            AssociationType::OptionHasOne => {
                quote! {
                    node.#root_model_field.#foreign_key_field == Some((child.0).#field_root_model_field.id)
                }
            }
            AssociationType::HasMany => {
                if data.foreign_key_optional {
                    quote! {
                        Some(node.#root_model_field.id) ==
                            (child.0).#field_root_model_field.#foreign_key_field
                    }
                } else {
                    quote! {
                        node.#root_model_field.id ==
                            (child.0).#field_root_model_field.#foreign_key_field
                    }
                }
            }
            AssociationType::HasManyThrough => {
                quote! {
                    let join_model = &child.1;
                    let association = &child.0;

                    node.#root_model_field.id == join_model.#foreign_key_field &&
                        join_model.#model_id_field == association.#model_field.id
                }
            }
        };

        quote! {
            fn is_child_of(node: &Self, child: &(#inner_type, &#join_model)) -> bool {
                #is_child_of_impl
            }
        }
    }

    fn child_id(&self, data: &FieldDeriveData) -> TokenStream {
        match data.association_type {
            AssociationType::HasOne => {
                quote! { Self::Id }
            }
            AssociationType::OptionHasOne => {
                quote! { Option<Self::Id> }
            }
            AssociationType::HasMany => {
                quote! { Vec<Self::Id> }
            }
            AssociationType::HasManyThrough => {
                quote! { Vec<Self::Id> }
            }
        }
    }

    fn loaded_or_failed_child_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let field_name = &data.field_name;
        let inner_type = &data.inner_type;

        quote! {
            fn loaded_child(node: &mut Self, child: #inner_type) {
                node.#field_name.loaded(child)
            }
        }
    }

    fn assert_loaded_otherwise_failed_impl(&self, data: &FieldDeriveData) -> TokenStream {
        let field_name = &data.field_name;

        quote! {
            fn assert_loaded_otherwise_failed(node: &mut Self) {
                node.#field_name.assert_loaded_otherwise_failed();
            }
        }
    }

    fn gen_eager_load_all_children(&mut self) {
        let struct_name = self.struct_name();

        let eager_load_children_calls = self
            .struct_fields()
            .filter_map(|field| self.gen_eager_load_all_children_for_field(field));

        self.tokens.extend(quote! {
            impl<'a> juniper_eager_loading::EagerLoadAllChildren<
                QueryTrail<'a, Self, juniper_from_schema::Walked>
            > for #struct_name {
                fn eager_load_all_children_for_each(
                    nodes: &mut [Self],
                    models: &[Self::Model],
                    db: &Self::Connection,
                    trail: &QueryTrail<'a, Self, juniper_from_schema::Walked>,
                ) -> Result<(), Self::Error> {
                    #(#eager_load_children_calls)*

                    Ok(())
                }
            }
        });
    }

    fn gen_eager_load_all_children_for_field(&self, field: &syn::Field) -> Option<TokenStream> {
        let inner_type = get_type_from_association(&field.ty)?;

        let (args, _data) = self.parse_field_args(field)?;

        let field_name = args
            .graphql_field()
            .clone()
            .map(|ident| {
                let ident = ident.to_string().to_snake_case();
                Ident::new(&ident, Span::call_site())
            })
            .unwrap_or_else(|| {
                field.ident.clone().unwrap_or_else(|| {
                    panic!("Found `juniper_eager_loading::HasOne` field without a name")
                })
            });

        let context = self.field_context_name(&field);

        Some(quote! {
            if let Some(trail) = trail.#field_name().walk() {
                EagerLoadChildrenOfType::<#inner_type, _, #context, _>::eager_load_children(
                    nodes,
                    models,
                    db,
                    &trail,
                )?;
            }
        })
    }

    fn struct_name(&self) -> &syn::Ident {
        &self.input.ident
    }

    fn model(&self) -> TokenStream {
        self.args.model(&self.struct_name())
    }

    fn id(&self) -> TokenStream {
        self.args.id()
    }

    fn connection(&self) -> TokenStream {
        self.args.connection()
    }

    fn error(&self) -> TokenStream {
        self.args.error()
    }

    fn root_model_field(&self) -> TokenStream {
        self.args.root_model_field(&self.struct_name())
    }

    fn struct_fields(&self) -> syn::punctuated::Iter<syn::Field> {
        use syn::{Data, Fields};

        match &self.input.data {
            Data::Union(_) => panic!("Factory can only be derived on structs"),
            Data::Enum(_) => panic!("Factory can only be derived on structs"),
            Data::Struct(data) => match &data.fields {
                Fields::Named(named) => named.named.iter(),
                Fields::Unit => panic!("Factory can only be derived on structs with named fields"),
                Fields::Unnamed(_) => {
                    panic!("Factory can only be derived on structs with named fields")
                }
            },
        }
    }

    fn field_context_name(&self, field: &syn::Field) -> Ident {
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
    let pair = if_let_or_none!(Some, segments.last());
    let segment = pair.value();
    let args = if_let_or_none!(PathArguments::AngleBracketed, &segment.arguments);
    let pair = if_let_or_none!(Some, args.args.last());
    let ty = if_let_or_none!(GenericArgument::Type, pair.value());

    Some(ty)
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
    let pair = if_let_or_none!(Some, segments.last());
    let segment = pair.value();
    Some(&segment.ident)
}

fn parse_field_args<T: FromMeta>(field: &syn::Field) -> Result<T, darling::Error> {
    let attrs = field
        .attrs
        .iter()
        .map(|attr| {
            let meta = attr.parse_meta().unwrap_or_else(|e| panic!("{}", e));
            NestedMeta::from(meta)
        })
        .collect::<Vec<_>>();
    FromMeta::from_list(attrs.as_slice())
}

#[derive(Debug)]
#[allow(dead_code)]
struct FieldDeriveData {
    foreign_key_field: TokenStream,
    foreign_key_optional: bool,
    field_root_model_field: TokenStream,
    root_model_field: TokenStream,
    join_model: TokenStream,
    inner_type: syn::Type,
    field_name: Ident,
    association_type: AssociationType,
    model_field: TokenStream,
    join_model_field: TokenStream,
    predicate_method: Option<Ident>,
}

impl FieldDeriveData {
    fn model_id_field(&self) -> Ident {
        Ident::new(&format!("{}_id", self.model_field), Span::call_site())
    }
}
