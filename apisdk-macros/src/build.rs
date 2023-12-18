use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{Attribute, Visibility};

use crate::parse::{parse_meta, ApiMeta};

/// Generate ApiBuilder
pub(crate) fn build_builder(
    vis: Visibility,
    api_name: Ident,
    meta: proc_macro::TokenStream,
    fields_init: TokenStream,
) -> (Ident, TokenStream) {
    let name = Ident::new(format!("{}Builder", api_name).as_str(), Span::call_site());
    let ApiMeta { base_uri } = parse_meta(meta);

    let builder = quote! {
        /// The build is used to customize the api
        #vis struct #name {
            inner: apisdk::ApiBuilder,
        }

        impl #name {
            /// Construct a new builder with base_uri
            fn new() -> Self {
                Self {
                    inner: apisdk::ApiBuilder::new(#base_uri).expect("Invalid base_uri"),
                }
            }

            /// Set ApiRouter
            #vis fn with_router(self, router: impl apisdk::ApiRouter) -> Self {
                Self {
                    inner: self.inner.with_router(router)
                }
            }

            /// Set ApiSignature
            #vis fn with_signature(self, signature: impl apisdk::ApiSignature) -> Self {
                Self {
                    inner: self.inner.with_signature(signature)
                }
            }

            /// Set initialiser
            #vis fn with_initialiser(self, initialiser: impl apisdk::Initialiser) -> Self {
                Self {
                    inner: self.inner.with_initialiser(initialiser)
                }
            }

            /// Add middleware
            #vis fn with_middleware(self, middleware: impl apisdk::Middleware) -> Self {
                Self {
                    inner: self.inner.with_middleware(middleware)
                }
            }

            /// Enable/disable log
            #vis fn with_log(self, enabled: bool) -> Self {
                Self {
                    inner: self.inner.with_initialiser(apisdk::LogConfig::new(enabled))
                }
            }

            /// Build the api instance
            #vis fn build(self) -> #api_name {
                let core = self.inner.build();
                #api_name {
                    core: std::sync::Arc::new(core),
                    #fields_init
                }
            }
        }
    };

    (name, builder)
}

/// Generate api basic implemations
pub(crate) fn build_api_impl(
    vis: Visibility,
    api_name: Ident,
    api_attrs: Vec<Attribute>,
    fields_decl: TokenStream,
    builder_name: Ident,
) -> TokenStream {
    quote! {
        #(#api_attrs)*
        #vis struct #api_name {
            core: std::sync::Arc<apisdk::ApiCore>,
            #fields_decl
        }

        impl Default for #api_name {
            fn default() -> Self {
                Self::builder().build()
            }
        }

        impl #api_name {
            thread_local! {
                #vis static REQ_CONFIG: std::cell::RefCell<apisdk::internal::RequestConfigurator>
                    = std::cell::RefCell::new(apisdk::internal::RequestConfigurator::default());
            }

            /// Create an ApiBuilder
            #vis fn builder() -> #builder_name {
                #builder_name::new()
            }

            /// Build request url
            /// - path: relative path
            #vis async fn build_url(
                &self,
                path: impl AsRef<str>,
            ) -> apisdk::ApiResult<apisdk::Url> {
                self.core.build_url(path).await
            }

            /// Build a new HTTP request
            /// - method: HTTP method
            /// - path: relative path
            #vis async fn request(
                &self,
                method: apisdk::Method,
                path: impl AsRef<str>,
            ) -> apisdk::ApiResult<apisdk::RequestBuilder> {
                self.core.build_request(method, path).await
            }
        }
    }
}

/// Generate shortcut methods for api
pub(crate) fn build_api_methods(vis: Visibility) -> Vec<TokenStream> {
    [
        "head", "get", "post", "put", "patch", "delete", "options", "trace",
    ]
    .iter()
    .map(|method| {
        let method_func = Ident::new(method, Span::call_site());
        let method_enum = Ident::new(&method.to_uppercase(), Span::call_site());
        quote! {
            /// Build a new HTTP request
            /// - path: relative path
            #vis async fn #method_func(
                &self,
                path: impl AsRef<str>,
            ) -> apisdk::ApiResult<apisdk::RequestBuilder> {
                use std::str::FromStr;
                self.core.build_request(apisdk::Method::#method_enum, path).await
            }
        }
    })
    .collect()
}

pub(crate) fn build_macro_overrides(_fn_name: Ident) -> Vec<TokenStream> {
    // let fn_name = fn_name.to_string();
    ["send", "send_json", "send_form", "send_multipart"]
        .iter()
        .map(|name| {
            let macro_name = Ident::new(name, Span::call_site());
            let macro_with_name = Ident::new(format!("_{}_with", name).as_str(), Span::call_site());
            quote! {
                #[allow(unused)]
                macro_rules! #macro_name {
                    ($req:expr) => {
                        async {
                            apisdk::#macro_with_name!($req, Self::REQ_CONFIG.take()).await
                        }
                    };
                    ($req:expr, $arg:tt) => {
                        async {
                            apisdk::#macro_with_name!($req, $arg, Self::REQ_CONFIG.take()).await
                        }
                    };
                    ($req:expr, $arg1:expr, $arg2:tt) => {
                        async {
                            apisdk::#macro_with_name!($req, $arg1, $arg2, Self::REQ_CONFIG.take()).await
                        }
                    };
                }
            }
        })
        .collect()
}
