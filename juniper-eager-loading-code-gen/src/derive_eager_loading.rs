mod field_args;

use darling::{FromDeriveInput, FromMeta};
use field_args::{HasManyType, DeriveArgs, FieldArgs, HasOne, OptionHasOne, HasMany};
use heck::CamelCase;
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
        let inner_type = get_type_from_association(&field.ty)?;

        let struct_name = self.struct_name();

        let is_has_many = is_has_many(&field.ty)?;
        let is_option_has_one = is_option_has_one(&field.ty)?;

        let args = if is_has_many {
            let args = parse_field_args::<HasMany>(&field)
                .unwrap_or_else(|e| panic!("{}", e))
                .has_many;
            FieldArgs::from(args)
        } else if is_option_has_one {
            let args = parse_field_args::<OptionHasOne>(&field)
                .unwrap_or_else(|e| panic!("{}", e))
                .option_has_one;
            FieldArgs::from(args)
        } else {
            let args = parse_field_args::<HasOne>(&field)
                .unwrap_or_else(|e| panic!("{}", e))
                .has_one;
            FieldArgs::from(args)
        };

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::HasOne` field without a name")
        });

        let foreign_key_field_default = if is_has_many {
            self.struct_name()
        } else {
            &field_name
        };

        let data = FieldDeriveData {
            field_name,
            inner_type,
            root_model_field: &self.root_model_field(),
            foreign_key_field: args.foreign_key_field(foreign_key_field_default),
            field_root_model_field: args.root_model_field(&field_name),
            association_type: args.association_type(),
            is_has_many,
            is_option_has_one,
        };

        let child_model = args.model(&inner_type);
        let child_id = self.child_id(&data);
        let child_ids_impl = self.child_ids_impl(&data);
        let load_children_impl = self.load_children_impl(&data);
        let load_from_cache_impl = self.load_from_cache_impl(&data);
        let is_child_of_impl = self.is_child_of_impl(&data);
        let loaded_or_failed_child_impl = self.loaded_or_failed_child_impl(&data);
        let store_in_cache_impl = self.store_in_cache_impl(&data);

        let context = self.field_context_name(&field);

        Some(quote! {
            #[allow(missing_doc, dead_code)]
            struct #context;

            impl<'a> EagerLoadChildrenOfType<
                #inner_type,
                QueryTrail<'a, #inner_type, juniper_from_schema::Walked>,
                #context,
            > for #struct_name {
                type ChildModel = #child_model;
                type ChildId = #child_id;

                #child_ids_impl
                #load_children_impl
                #load_from_cache_impl
                #is_child_of_impl
                #loaded_or_failed_child_impl
                #store_in_cache_impl
            }
        })
    }

    fn child_ids_impl(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        let foreign_key_field = &data.foreign_key_field;

        let child_ids_from_field = quote! {
            let ids = models
                .iter()
                .map(|model| model.#foreign_key_field.clone())
                .collect::<Vec<_>>();
            Ok(juniper_eager_loading::LoadResult::Ids(ids))
        };

        let child_ids_impl = if data.is_has_many {
            let association_type = data.association_type();

            match association_type {
                HasManyType::OneToMany => child_ids_from_field,
                HasManyType::ManyToMany => {
                    quote! {
                        let models = <
                            Self::ChildModel
                            as
                            juniper_eager_loading::LoadFromModels<Self::Model>
                        >::load(
                            models,
                            db,
                        )?;
                        Ok(juniper_eager_loading::LoadResult::Models(models))
                    }
                }
            }
        } else {
            child_ids_from_field
        };

        quote! {
            #[allow(unused_variables)]
            fn child_ids(
                models: &[Self::Model],
                db: &Self::Connection,
            ) -> Result<
                juniper_eager_loading::LoadResult<Self::ChildModel, Self::ChildId>,
                Self::Error,
            > {
                #child_ids_impl
            }
        }
    }

    fn load_children_impl(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        let normalize_ids = self.normalize_ids(data);

        quote! {
            fn load_children(
                ids: &[Self::ChildId],
                db: &Self::Connection,
            ) -> Result<Vec<Self::ChildModel>, Self::Error> {
                #normalize_ids
                <Self::ChildModel as juniper_eager_loading::LoadFromIds>::load(&ids, db)
            }
        }
    }

    fn normalize_ids(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        if data.is_option_has_one {
            quote! {
                let ids = ids
                    .into_iter()
                    .filter_map(|id| id .as_ref())
                    .cloned()
                    .collect::<Vec<_>>();
            }
        } else if data.is_has_many {
            quote! {
                let ids = ids.iter().flatten().cloned().collect::<Vec<_>>();
            }
        } else {
            quote! {}
        }
    }

    fn load_from_cache_impl(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        let normalize_ids = self.normalize_ids(data);

        let load_from_cache_impl = if data.is_option_has_one {
            quote! {
                #normalize_ids
                ids.into_iter().map(|id| {
                    if let Some(model) = cache.get::<Self::ChildModel, _>(id) {
                        juniper_eager_loading::CacheLoadResult::Loaded(
                            std::clone::Clone::clone(model)
                        )
                    } else {
                        juniper_eager_loading::CacheLoadResult::Missing(Some(id))
                    }
                }).collect::<Vec<_>>()
            }
        } else if data.is_has_many {
            quote! {
                #normalize_ids
                ids.into_iter().map(|id| {
                    if let Some(model) = cache.get::<Self::ChildModel, _>(id) {
                        juniper_eager_loading::CacheLoadResult::Loaded(
                            std::clone::Clone::clone(model)
                        )
                    } else {
                        let mut missing = Vec::with_capacity(1);
                        missing.push(id);
                        juniper_eager_loading::CacheLoadResult::Missing(missing)
                    }
                }).collect::<Vec<_>>()
            }
        } else {
            quote! {
                ids.into_iter().cloned().map(|id| {
                    if let Some(model) = cache.get::<Self::ChildModel, _>(id) {
                        juniper_eager_loading::CacheLoadResult::Loaded(
                            std::clone::Clone::clone(model)
                        )
                    } else {
                        juniper_eager_loading::CacheLoadResult::Missing(id)
                    }
                }).collect::<Vec<_>>()
            }
        };

        quote! {
            fn load_from_cache(
                ids: &[Self::ChildId],
                cache: &juniper_eager_loading::Cache<Self::Id>,
            ) -> Vec<
                juniper_eager_loading::CacheLoadResult<Self::ChildModel, Self::ChildId>
            > {
                #load_from_cache_impl
            }
        }
    }

    fn is_child_of_impl(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        let root_model_field = &data.root_model_field;
        let foreign_key_field = &data.foreign_key_field;
        let field_root_model_field = &data.field_root_model_field;
        let inner_type = &data.inner_type;

        let is_child_of_impl = if data.is_option_has_one {
            quote! {
                node.#root_model_field.#foreign_key_field == Some(child.#field_root_model_field.id)
            }
        } else if data.is_has_many {
            let association_type = data.association_type();

            match association_type {
                HasManyType::OneToMany => {
                    quote! {
                        node
                            .#root_model_field
                            .#foreign_key_field
                            .contains(&child.#field_root_model_field.id)
                    }
                }
                HasManyType::ManyToMany => {
                    quote! {
                        node.#root_model_field.id ==
                            child.#field_root_model_field.#foreign_key_field
                    }
                }
            }
        } else {
            quote! {
                node.#root_model_field.#foreign_key_field == child.#field_root_model_field.id
            }
        };

        quote! {
            fn is_child_of(node: &Self, child: &#inner_type) -> bool {
                #is_child_of_impl
            }
        }
    }

    fn child_id(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        if data.is_option_has_one {
            quote! { Option<Self::Id> }
        } else if data.is_has_many {
            quote! { Vec<Self::Id> }
        } else {
            quote! { Self::Id }
        }
    }

    fn loaded_or_failed_child_impl(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        let field_name = &data.field_name;
        let inner_type = &data.inner_type;

        quote! {
            fn loaded_or_failed_child(node: &mut Self, child: Option<&#inner_type>) {
                node.#field_name.loaded_or_failed(child.cloned())
            }
        }
    }

    fn store_in_cache_impl(&self, _: &FieldDeriveData<'_>) -> TokenStream {
        quote! {
            fn store_in_cache(
                child: &Self::ChildModel,
                cache: &mut juniper_eager_loading::Cache<Self::Id>,
            ) {
                cache.insert::<Self::ChildModel, _>(child.id, child.clone());
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
                    cache: &mut juniper_eager_loading::Cache<Self::Id>,
                ) -> Result<(), Self::Error> {
                    #(#eager_load_children_calls)*

                    Ok(())
                }
            }
        });
    }

    fn gen_eager_load_all_children_for_field(&self, field: &syn::Field) -> Option<TokenStream> {
        let inner_type = get_type_from_association(&field.ty)?;

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::HasOne` field without a name")
        });

        let context = self.field_context_name(&field);

        Some(quote! {
            if let Some(trail) = trail.#field_name().walk() {
                EagerLoadChildrenOfType::<#inner_type, _, #context>::eager_load_children(
                    nodes,
                    models,
                    db,
                    &trail,
                    cache,
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

#[derive(Debug, Eq, PartialEq)]
enum AssociationType {
    HasOne,
    OptionHasOne,
    HasMany,
}

fn association_type(ty: &syn::Type) -> Option<AssociationType> {
    if *last_ident_in_type_segment(ty)? == "OptionHasOne" {
        return Some(AssociationType::OptionHasOne);
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

fn is_option_has_one(ty: &syn::Type) -> Option<bool> {
    Some(association_type(ty)? == AssociationType::OptionHasOne)
}

fn is_has_many(ty: &syn::Type) -> Option<bool> {
    Some(association_type(ty)? == AssociationType::HasMany)
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

#[allow(dead_code)]
fn type_to_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    let mut tokenized = quote! {};
    ty.to_tokens(&mut tokenized);
    tokenized.to_string()
}

struct FieldDeriveData<'a> {
    association_type: HasManyType,
    foreign_key_field: TokenStream,
    field_root_model_field: TokenStream,
    root_model_field: &'a TokenStream,
    inner_type: &'a syn::Type,
    field_name: &'a Ident,
    is_option_has_one: bool,
    is_has_many: bool,
}

impl<'a> FieldDeriveData<'a> {
    fn association_type(&self) -> HasManyType {
        self.association_type
    }
}
