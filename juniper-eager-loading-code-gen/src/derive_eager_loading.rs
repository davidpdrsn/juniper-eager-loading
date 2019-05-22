use darling::{FromDeriveInput, FromMeta};
use lazy_static::lazy_static;
use proc_macro2::{Span, TokenStream};
use quote::quote;
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

            if is_db_edge_field(&field.ty).unwrap() {
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
        let root_model_field = self.root_model_field();

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::DbEdge` field without a name")
        });

        let args = parse_field_args::<DbEdgeFieldOptions>(&field).unwrap();

        let foreign_key_field = &args.foreign_key_field(&field_name);

        let is_option_db_edge = is_option_db_edge(&field.ty)?;

        let child_model = &args.model;

        let child_id = if is_option_db_edge {
            quote! { Option<Self::Id> }
        } else {
            quote! { Self::Id }
        };

        let is_child_of_impl = if is_option_db_edge {
            quote! {
                node.#root_model_field.#foreign_key_field == Some(child.#field_name.id)
            }
        } else {
            quote! {
                node.#root_model_field.#foreign_key_field == child.#field_name.id
            }
        };

        let load_children_impl = if is_option_db_edge {
            quote! {
                let ids = ids
                    .into_iter()
                    .filter_map(|id| id.as_ref())
                    .map(|id| std::clone::Clone::clone(id))
                    .collect::<Vec<_>>();
                <Self::ChildModel as juniper_eager_loading::LoadFromIds>::load(&ids, db)
            }
        } else {
            quote! {
                <Self::ChildModel as juniper_eager_loading::LoadFromIds>::load(ids, db)
            }
        };

        Some(quote! {
            impl<'a> EagerLoadChildrenOfType<
                #inner_type,
                QueryTrail<'a, #inner_type, juniper_from_schema::Walked>,
            > for #struct_name {
                type ChildModel = #child_model;
                type ChildId = #child_id;

                fn child_id(model: &Self::Model) -> Self::ChildId {
                    model.#foreign_key_field
                }

                fn load_children(
                    ids: &[Self::ChildId],
                    db: &Self::Connection,
                ) -> Result<Vec<Self::ChildModel>, Self::Error> {
                    #load_children_impl
                }

                fn is_child_of(node: &Self, child: &#inner_type) -> bool {
                    #is_child_of_impl
                }

                fn loaded_or_failed_child(node: &mut Self, child: Option<&#inner_type>) {
                    node.#field_name.loaded_or_failed(child.cloned())
                }
            }
        })
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
        let inner_type = get_type_from_db_edge(&field.ty)?;

        let field_name = field.ident.as_ref().unwrap_or_else(|| {
            panic!("Found `juniper_eager_loading::DbEdge` field without a name")
        });

        Some(quote! {
            if let Some(trail) = trail.#field_name().walk() {
                EagerLoadChildrenOfType::<#inner_type, _>::eager_load_children(
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
        use heck::SnakeCase;

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
    if !is_db_edge_field(ty)? {
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

fn is_db_edge_field(ty: &syn::Type) -> Option<bool> {
    let res = *last_ident_in_type_segment(ty)? == "DbEdge"
        || *last_ident_in_type_segment(ty)? == "OptionDbEdge";
    Some(res)
}

fn is_option_db_edge(ty: &syn::Type) -> Option<bool> {
    let res = *last_ident_in_type_segment(ty)? == "OptionDbEdge";
    Some(res)
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
