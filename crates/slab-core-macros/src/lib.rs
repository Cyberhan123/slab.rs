use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse_quote;
use syn::spanned::Spanned;
use syn::{Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, Path, Result};

#[proc_macro_attribute]
pub fn backend_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_backend_handler(item) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error().into(),
    }
}

struct EventRoute {
    pattern: Path,
    method: syn::Ident,
}

struct RuntimeRoute {
    pattern: Path,
    method: syn::Ident,
    pass_signal: bool,
}

struct PeerRoute {
    pattern: Path,
    method: syn::Ident,
    pass_cmd: bool,
}

fn helper_attr_name(attr: &Attribute) -> Option<&'static str> {
    if attr.path().is_ident("on_event") {
        Some("on_event")
    } else if attr.path().is_ident("on_runtime_control") {
        Some("on_runtime_control")
    } else if attr.path().is_ident("on_peer_control") {
        Some("on_peer_control")
    } else if attr.path().is_ident("on_control_lagged") {
        Some("on_control_lagged")
    } else {
        None
    }
}

fn strip_helper_attrs(attrs: &mut Vec<Attribute>) {
    attrs.retain(|attr| helper_attr_name(attr).is_none());
}

fn normalize_event_path(path: Path) -> Path {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        let ident = path.segments[0].ident.clone();
        parse_quote!(crate::internal::scheduler::backend::protocol::RequestRoute::#ident)
    } else {
        path
    }
}

fn normalize_runtime_control_path(path: Path) -> Path {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        let ident = path.segments[0].ident.clone();
        parse_quote!(crate::internal::scheduler::backend::protocol::RuntimeControlSignal::#ident)
    } else {
        path
    }
}

fn normalize_peer_control_path(path: Path) -> Path {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        let ident = path.segments[0].ident.clone();
        parse_quote!(crate::internal::scheduler::backend::protocol::PeerWorkerCommand::#ident)
    } else {
        path
    }
}

fn non_receiver_arg_count(method: &ImplItemFn) -> usize {
    method.sig.inputs.iter().filter(|arg| !matches!(arg, FnArg::Receiver(_))).count()
}

fn expand_backend_handler(item: TokenStream) -> Result<TokenStream> {
    let mut item_impl = syn::parse::<ItemImpl>(item)?;
    if item_impl.trait_.is_some() {
        return Err(syn::Error::new(
            item_impl.impl_token.span,
            "#[backend_handler] only supports inherent impl blocks",
        ));
    }

    let mut event_routes: Vec<EventRoute> = Vec::new();
    let mut runtime_routes: Vec<RuntimeRoute> = Vec::new();
    let mut peer_routes: Vec<PeerRoute> = Vec::new();
    let mut seen_event_keys = HashSet::<String>::new();
    let mut seen_runtime_keys = HashSet::<String>::new();
    let mut seen_peer_keys = HashSet::<String>::new();
    let mut peer_fallback_method: Option<(syn::Ident, bool)> = None;
    let mut control_lagged_method: Option<syn::Ident> = None;

    for item in &mut item_impl.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let method_ident = method.sig.ident.clone();

        let mut event_args = Vec::<Path>::new();
        let mut runtime_args = Vec::<Path>::new();
        let mut peer_args = Vec::<Option<Path>>::new();
        let mut has_control_lagged = false;

        for attr in &method.attrs {
            match helper_attr_name(attr) {
                Some("on_event") => {
                    event_args.push(attr.parse_args::<Path>()?);
                }
                Some("on_runtime_control") => {
                    runtime_args.push(attr.parse_args::<Path>()?);
                }
                Some("on_peer_control") => {
                    let arg = match &attr.meta {
                        syn::Meta::Path(_) => None,
                        _ => Some(attr.parse_args::<Path>()?),
                    };
                    peer_args.push(arg);
                }
                Some("on_control_lagged") => {
                    has_control_lagged = true;
                }
                _ => {}
            }
        }

        strip_helper_attrs(&mut method.attrs);

        if event_args.is_empty()
            && runtime_args.is_empty()
            && peer_args.is_empty()
            && !has_control_lagged
        {
            continue;
        }

        if method.sig.asyncness.is_none() {
            return Err(syn::Error::new(
                method.sig.span(),
                "backend handler methods must be async",
            ));
        }

        if !event_args.is_empty() && non_receiver_arg_count(method) != 1 {
            return Err(syn::Error::new(
                method.sig.inputs.span(),
                "event handlers must take exactly one non-self argument (BackendRequest)",
            ));
        }

        if !runtime_args.is_empty() {
            let count = non_receiver_arg_count(method);
            if count > 1 {
                return Err(syn::Error::new(
                    method.sig.inputs.span(),
                    "runtime control handlers may take zero or one non-self argument",
                ));
            }
            let pass_signal = count == 1;
            for raw in runtime_args {
                let pattern = normalize_runtime_control_path(raw);
                let key = quote!(#pattern).to_string();
                if !seen_runtime_keys.insert(key.clone()) {
                    return Err(syn::Error::new(
                        method_ident.span(),
                        format!("duplicate runtime control handler for `{key}`"),
                    ));
                }
                runtime_routes.push(RuntimeRoute {
                    pattern,
                    method: method_ident.clone(),
                    pass_signal,
                });
            }
        }

        for raw in event_args {
            let pattern = normalize_event_path(raw);
            let key = quote!(#pattern).to_string();
            if !seen_event_keys.insert(key.clone()) {
                return Err(syn::Error::new(
                    method_ident.span(),
                    format!("duplicate event handler for `{key}`"),
                ));
            }
            event_routes.push(EventRoute { pattern, method: method_ident.clone() });
        }

        if !peer_args.is_empty() {
            let count = non_receiver_arg_count(method);
            if count > 1 {
                return Err(syn::Error::new(
                    method.sig.inputs.span(),
                    "peer control handler may take zero or one non-self argument",
                ));
            }
            let pass_cmd = count == 1;
            for raw in peer_args {
                if let Some(raw_path) = raw {
                    let pattern = normalize_peer_control_path(raw_path);
                    let key = quote!(#pattern).to_string();
                    if !seen_peer_keys.insert(key.clone()) {
                        return Err(syn::Error::new(
                            method_ident.span(),
                            format!("duplicate peer control handler for `{key}`"),
                        ));
                    }
                    peer_routes.push(PeerRoute { pattern, method: method_ident.clone(), pass_cmd });
                } else {
                    if peer_fallback_method.is_some() {
                        return Err(syn::Error::new(
                            method_ident.span(),
                            "only one bare #[on_peer_control] fallback handler is allowed",
                        ));
                    }
                    peer_fallback_method = Some((method_ident.clone(), pass_cmd));
                }
            }
        }

        if has_control_lagged {
            if control_lagged_method.is_some() {
                return Err(syn::Error::new(
                    method_ident.span(),
                    "only one #[on_control_lagged] handler is allowed",
                ));
            }
            if non_receiver_arg_count(method) != 0 {
                return Err(syn::Error::new(
                    method.sig.inputs.span(),
                    "control lagged handler must not take non-self arguments",
                ));
            }
            control_lagged_method = Some(method_ident.clone());
        }
    }

    let event_generated = event_routes.iter().enumerate().map(|(idx, route)| {
        let matcher = format_ident!("__backend_handler_match_event_{}", idx);
        let caller = format_ident!("__backend_handler_call_event_{}", idx);
        let pattern = &route.pattern;
        let method = &route.method;
        quote! {
            fn #matcher(route: crate::internal::scheduler::backend::protocol::RequestRoute) -> bool {
                matches!(route, #pattern)
            }
            fn #caller<'a>(
                &'a mut self,
                req: crate::internal::scheduler::backend::protocol::BackendRequest,
            ) -> crate::internal::scheduler::backend::runner::HandlerFuture<'a> {
                Box::pin(async move {
                    self.#method(req).await;
                })
            }
        }
    });

    let event_table_entries = (0..event_routes.len()).map(|idx| {
        let matcher = format_ident!("__backend_handler_match_event_{}", idx);
        let caller = format_ident!("__backend_handler_call_event_{}", idx);
        quote! {
            crate::internal::scheduler::backend::runner::RequestRouteMatcher {
                matches: Self::#matcher,
                handle: Self::#caller,
            }
        }
    });

    let runtime_generated = runtime_routes.iter().enumerate().map(|(idx, route)| {
        let matcher = format_ident!("__backend_handler_match_runtime_{}", idx);
        let caller = format_ident!("__backend_handler_call_runtime_{}", idx);
        let pattern = &route.pattern;
        let method = &route.method;
        let call = if route.pass_signal {
            quote! { self.#method(signal).await; }
        } else {
            quote! { self.#method().await; }
        };
        quote! {
            fn #matcher(sig: &crate::internal::scheduler::backend::protocol::RuntimeControlSignal) -> bool {
                matches!(sig, #pattern { .. })
            }
            fn #caller<'a>(
                &'a mut self,
                signal: crate::internal::scheduler::backend::protocol::RuntimeControlSignal,
            ) -> crate::internal::scheduler::backend::runner::HandlerFuture<'a> {
                Box::pin(async move {
                    #call
                })
            }
        }
    });

    let runtime_table_entries = (0..runtime_routes.len()).map(|idx| {
        let matcher = format_ident!("__backend_handler_match_runtime_{}", idx);
        let caller = format_ident!("__backend_handler_call_runtime_{}", idx);
        quote! {
            crate::internal::scheduler::backend::runner::RuntimeRoute {
                matches: Self::#matcher,
                handle: Self::#caller,
            }
        }
    });

    let peer_variant_generated = peer_routes.iter().enumerate().map(|(idx, route)| {
        let matcher = format_ident!("__backend_handler_match_peer_{}", idx);
        let caller = format_ident!("__backend_handler_call_peer_route_{}", idx);
        let pattern = &route.pattern;
        let method = &route.method;
        let call = if route.pass_cmd {
            quote! { self.#method(cmd).await; }
        } else {
            quote! { self.#method().await; }
        };
        quote! {
            fn #matcher(cmd: &crate::internal::scheduler::backend::protocol::PeerWorkerCommand) -> bool {
                matches!(cmd, #pattern { .. })
            }
            fn #caller<'a>(
                &'a mut self,
                cmd: crate::internal::scheduler::backend::protocol::PeerWorkerCommand,
            ) -> crate::internal::scheduler::backend::runner::HandlerFuture<'a> {
                Box::pin(async move {
                    #call
                })
            }
        }
    });

    let peer_table_entries = (0..peer_routes.len()).map(|idx| {
        let matcher = format_ident!("__backend_handler_match_peer_{}", idx);
        let caller = format_ident!("__backend_handler_call_peer_route_{}", idx);
        quote! {
            crate::internal::scheduler::backend::runner::PeerRoute {
                matches: Self::#matcher,
                handle: Self::#caller,
            }
        }
    });

    let peer_fallback_generated = if let Some((method, pass_cmd)) = peer_fallback_method.as_ref() {
        let call = if *pass_cmd {
            quote! { self.#method(cmd).await; }
        } else {
            quote! { self.#method().await; }
        };
        quote! {
            fn __backend_handler_call_peer<'a>(
                &'a mut self,
                cmd: crate::internal::scheduler::backend::protocol::PeerWorkerCommand,
            ) -> crate::internal::scheduler::backend::runner::HandlerFuture<'a> {
                Box::pin(async move {
                    #call
                })
            }
        }
    } else {
        quote! {}
    };

    let peer_fallback_route = if peer_fallback_method.is_some() {
        quote! { Some(Self::__backend_handler_call_peer as crate::internal::scheduler::backend::runner::PeerDispatchFn<Self>) }
    } else {
        quote! { None }
    };

    let lagged_generated = if let Some(method) = &control_lagged_method {
        quote! {
            fn __backend_handler_call_lagged<'a>(
                &'a mut self,
            ) -> crate::internal::scheduler::backend::runner::HandlerFuture<'a> {
                Box::pin(async move {
                    self.#method().await;
                })
            }
        }
    } else {
        quote! {}
    };

    let lagged_route = if control_lagged_method.is_some() {
        quote! { Some(Self::__backend_handler_call_lagged as crate::internal::scheduler::backend::runner::LaggedDispatchFn<Self>) }
    } else {
        quote! { None }
    };

    let self_ty = &item_impl.self_ty;
    let (impl_generics, _ty_generics, where_clause) = item_impl.generics.split_for_impl();

    let expanded = quote! {
        #item_impl

        impl #impl_generics #self_ty #where_clause {
            #(#event_generated)*
            #(#runtime_generated)*
            #(#peer_variant_generated)*
            #peer_fallback_generated
            #lagged_generated
        }

        #[async_trait::async_trait]
        impl #impl_generics crate::internal::scheduler::backend::runner::RuntimeWorkerHandler for #self_ty #where_clause {
            async fn handle_request(&mut self, req: crate::internal::scheduler::backend::protocol::BackendRequest) {
                crate::internal::scheduler::backend::runner::dispatch_backend_request(
                    self,
                    req,
                    &[#(#event_table_entries),*],
                ).await;
            }

            async fn handle_peer_control(
                &mut self,
                cmd: crate::internal::scheduler::backend::protocol::PeerWorkerCommand,
            ) {
                crate::internal::scheduler::backend::runner::dispatch_peer_control(
                    self,
                    cmd,
                    #peer_fallback_route,
                    &[#(#peer_table_entries),*],
                ).await;
            }

            async fn handle_runtime_control(
                &mut self,
                signal: crate::internal::scheduler::backend::protocol::RuntimeControlSignal,
            ) {
                crate::internal::scheduler::backend::runner::dispatch_runtime_control(
                    self,
                    signal,
                    &[#(#runtime_table_entries),*],
                ).await;
            }

            async fn handle_control_lagged(&mut self) {
                crate::internal::scheduler::backend::runner::dispatch_control_lagged(self, #lagged_route).await;
            }
        }
    };

    Ok(expanded.into())
}
