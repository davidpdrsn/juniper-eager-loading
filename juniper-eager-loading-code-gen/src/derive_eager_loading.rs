use darling::{FromDeriveInput, FromMeta};
use heck::{CamelCase, SnakeCase};
use lazy_static::lazy_static;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use syn::{
    parse_macro_input, DeriveInput, GenericArgument, Ident, NestedMeta, PathArguments, Type,
};

pub fn gen_tokens(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let options = match Options::from_derive_input(&ast) {
        Ok(options) => options,
        Err(err) => panic!("{}", err),
    };

    let out = DeriveData::new(ast, options);
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

#[derive(FromDeriveInput, Debug)]
#[darling(attributes(eager_loading), forward_attrs(doc, cfg, allow))]
struct Options {
    model: syn::Path,
    #[darling(default)]
    id: Option<syn::Path>,
    connection: syn::Path,
    error: syn::Path,
    #[darling(default)]
    root_model_field: Option<syn::Ident>,
}

struct DeriveData {
    input: DeriveInput,
    options: Options,
    tokens: TokenStream,
}

#[derive(FromMeta, Debug)]
struct DbEdgeFieldOptions {
    #[darling(default)]
    foreign_key_field: Option<syn::Ident>,
    model: syn::Path,
    #[darling(default)]
    root_model_field: Option<syn::Ident>,
    #[darling(default)]
    association_type: Option<AssociationType>,
}

#[derive(FromMeta, Copy, Clone, Debug)]
enum AssociationType {
    OneToMany,
    ManyToMany,
}

impl DbEdgeFieldOptions {
    fn foreign_key_field(&self, field_name: &Ident) -> TokenStream {
        self.foreign_key_field
            .as_ref()
            .map(|inner| quote! { #inner })
            .unwrap_or_else(|| {
                let field = field_name.to_string();
                let field = format!("{}_id", field);
                let field = Ident::new(&field, Span::call_site());
                quote! { #field }
            })
    }

    fn root_model_field(&self, field_name: &Ident) -> TokenStream {
        self.root_model_field
            .as_ref()
            .map(|ident| {
                quote! { #ident }
            })
            .unwrap_or_else(|| {
                quote! { #field_name }
            })
    }

    fn association_type(&self, field: &syn::Field) -> Option<AssociationType> {
        match (is_vec_db_edge(&field.ty)?, self.association_type) {
            (true, Some(ty)) => Some(ty),
            (true, None) => Some(AssociationType::OneToMany),

            (false, Some(_)) => {
                let mut f = String::new();
                writeln!(
                    f,
                    "Only `VecDbEdge` fields support `association_type` attributes"
                )
                .unwrap();
                writeln!(f, "Type was `{}`", type_to_string(&field.ty)).unwrap();
                panic!("{}", f);
            }
            (false, None) => None,
        }
    }
}

impl DeriveData {
    fn new(input: DeriveInput, options: Options) -> Self {
        Self {
            input,
            options,
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

            if is_db_edge_field(&field.ty) {
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
        let inner_type = get_type_from_db_edge(&field.ty)?;

        let struct_name = self.struct_name();

        let args = parse_field_args::<DbEdgeFieldOptions>(&field).unwrap();

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::DbEdge` field without a name")
        });

        let data = FieldDeriveData {
            field_name,
            inner_type,
            root_model_field: self.root_model_field(),
            foreign_key_field: args.foreign_key_field(&field_name),
            field_root_model_field: args.root_model_field(&field_name),
            association_type: args.association_type(field),
            is_vec_db_edge: is_vec_db_edge(&field.ty)?,
            is_option_db_edge: is_option_db_edge(&field.ty)?,
        };

        let child_model = &args.model;
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

        let child_ids_impl = if data.is_vec_db_edge {
            let association_type = data.association_type();

            match association_type {
                AssociationType::OneToMany => child_ids_from_field,
                AssociationType::ManyToMany => {
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
        if data.is_option_db_edge {
            quote! {
                let ids = ids
                    .into_iter()
                    .filter_map(|id| id .as_ref())
                    .cloned()
                    .collect::<Vec<_>>();
            }
        } else if data.is_vec_db_edge {
            quote! {
                let ids = ids.iter().flatten().cloned().collect::<Vec<_>>();
            }
        } else {
            quote! {}
        }
    }

    fn load_from_cache_impl(&self, data: &FieldDeriveData<'_>) -> TokenStream {
        let normalize_ids = self.normalize_ids(data);

        let load_from_cache_impl = if data.is_option_db_edge {
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
        } else if data.is_vec_db_edge {
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

        let is_child_of_impl = if data.is_option_db_edge {
            quote! {
                node.#root_model_field.#foreign_key_field == Some(child.#field_root_model_field.id)
            }
        } else if data.is_vec_db_edge {
            let association_type = data.association_type();

            match association_type {
                AssociationType::OneToMany => {
                    quote! {
                        node
                            .#root_model_field
                            .#foreign_key_field
                            .contains(&child.#field_root_model_field.id)
                    }
                }
                AssociationType::ManyToMany => {
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
        if data.is_option_db_edge {
            quote! { Option<Self::Id> }
        } else if data.is_vec_db_edge {
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
        let inner_type = get_type_from_db_edge(&field.ty)?;

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::DbEdge` field without a name")
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

    fn model(&self) -> &syn::Path {
        &self.options.model
    }

    fn id(&self) -> TokenStream {
        self.options
            .id
            .as_ref()
            .map(|inner| quote! { #inner })
            .unwrap_or_else(|| quote! { i32 })
    }

    fn connection(&self) -> &syn::Path {
        &self.options.connection
    }

    fn error(&self) -> &syn::Path {
        &self.options.error
    }

    fn root_model_field(&self) -> TokenStream {
        self.options
            .root_model_field
            .as_ref()
            .map(|inner| quote! { #inner })
            .unwrap_or_else(|| {
                let struct_name = self.struct_name().to_string().to_snake_case();
                let struct_name = Ident::new(&struct_name, Span::call_site());
                quote! { #struct_name }
            })
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

fn get_type_from_db_edge(ty: &syn::Type) -> Option<&syn::Type> {
    if !is_db_edge_field(ty) {
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
enum DbEdgeType {
    Bare,
    Option,
    Vec,
}

fn db_edge_type(ty: &syn::Type) -> Option<DbEdgeType> {
    if *last_ident_in_type_segment(ty)? == "OptionDbEdge" {
        return Some(DbEdgeType::Option);
    }

    if *last_ident_in_type_segment(ty)? == "VecDbEdge" {
        return Some(DbEdgeType::Vec);
    }

    if *last_ident_in_type_segment(ty)? == "DbEdge" {
        return Some(DbEdgeType::Bare);
    }

    None
}

fn is_db_edge_field(ty: &syn::Type) -> bool {
    db_edge_type(ty).is_some()
}

fn is_option_db_edge(ty: &syn::Type) -> Option<bool> {
    Some(db_edge_type(ty)? == DbEdgeType::Option)
}

fn is_vec_db_edge(ty: &syn::Type) -> Option<bool> {
    Some(db_edge_type(ty)? == DbEdgeType::Vec)
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
    #[derive(FromMeta)]
    struct FieldOptionsOuter<K> {
        eager_loading: K,
    }

    let attrs = field
        .attrs
        .iter()
        .map(|attr| {
            let meta = attr.parse_meta().unwrap();
            NestedMeta::from(meta)
        })
        .collect::<Vec<_>>();
    let outer: FieldOptionsOuter<T> = FromMeta::from_list(attrs.as_slice())?;
    Ok(outer.eager_loading)
}

fn type_to_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    let mut tokenized = quote! {};
    ty.to_tokens(&mut tokenized);
    tokenized.to_string()
}

struct FieldDeriveData<'a> {
    association_type: Option<AssociationType>,
    foreign_key_field: TokenStream,
    is_option_db_edge: bool,
    is_vec_db_edge: bool,
    field_root_model_field: TokenStream,
    root_model_field: TokenStream,
    inner_type: &'a syn::Type,
    field_name: &'a Ident,
}

impl<'a> FieldDeriveData<'a> {
    fn association_type(&self) -> AssociationType {
        self.association_type
            .unwrap_or_else(|| panic!("Missing attribute `association_type` for VecDbEdge"))
    }
}
