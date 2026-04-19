use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse_quote;
use syn::spanned::Spanned;
use syn::{
    Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, Path, Result, ReturnType, Type, Visibility,
};

#[derive(Clone)]
enum EventHandlerKind {
    Legacy,
    Typed { extractors: Vec<TokenStream2> },
}

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
    kind: EventHandlerKind,
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
        parse_quote!(::slab_runtime_core::backend::RequestRoute::#ident)
    } else {
        path
    }
}

fn normalize_runtime_control_path(path: Path) -> Path {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        let ident = path.segments[0].ident.clone();
        parse_quote!(::slab_runtime_core::backend::RuntimeControlSignal::#ident)
    } else {
        path
    }
}

fn normalize_peer_control_path(path: Path) -> Path {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        let ident = path.segments[0].ident.clone();
        parse_quote!(::slab_runtime_core::backend::PeerWorkerCommand::#ident)
    } else {
        path
    }
}

fn non_receiver_arg_count(method: &ImplItemFn) -> usize {
    method.sig.inputs.iter().filter(|arg| !matches!(arg, FnArg::Receiver(_))).count()
}

fn first_non_receiver_arg(method: &ImplItemFn) -> Option<&FnArg> {
    method.sig.inputs.iter().find(|arg| !matches!(arg, FnArg::Receiver(_)))
}

fn type_last_ident(ty: &Type) -> Option<&syn::Ident> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.path.segments.last().map(|segment| &segment.ident)
}

fn is_type_ident(ty: &Type, expected_ident: &str) -> bool {
    type_last_ident(ty).is_some_and(|ident| ident == expected_ident)
}

fn is_backend_request_arg(arg: &FnArg) -> bool {
    let FnArg::Typed(arg) = arg else {
        return false;
    };
    is_type_ident(arg.ty.as_ref(), "BackendRequest")
}

fn single_generic_type_arg(ty: &Type, wrapper_ident: &str) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != wrapper_ident {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner.clone()),
        _ => None,
    }
}

fn typed_event_handler_returns_result(method: &ImplItemFn) -> bool {
    match &method.sig.output {
        ReturnType::Type(_, ty) => is_type_ident(ty.as_ref(), "Result"),
        ReturnType::Default => false,
    }
}

fn event_arg_extractor(arg: &FnArg) -> Result<TokenStream2> {
    let FnArg::Typed(arg) = arg else {
        return Err(syn::Error::new(
            arg.span(),
            "event handlers may only use typed non-self arguments",
        ));
    };
    let ty = arg.ty.as_ref();
    if let Some(inner) = single_generic_type_arg(ty, "Input") {
        return Ok(quote! { ::slab_runtime_core::backend::extract_event_input::<#inner>(&req) });
    }
    if let Some(inner) = single_generic_type_arg(ty, "Options") {
        return Ok(quote! { ::slab_runtime_core::backend::extract_event_options::<#inner>(&req) });
    }
    if is_type_ident(ty, "String") {
        return Ok(quote! { ::slab_runtime_core::backend::extract_event_text(&req) });
    }
    if is_type_ident(ty, "Payload") {
        return Ok(quote! { ::slab_runtime_core::backend::extract_event_payload(&req) });
    }
    if is_type_ident(ty, "CancelRx") {
        return Ok(quote! { ::slab_runtime_core::backend::extract_event_cancel_rx(&req) });
    }
    if is_type_ident(ty, "BroadcastSeq") {
        return Ok(quote! { ::slab_runtime_core::backend::extract_event_broadcast_seq(&req) });
    }
    Err(syn::Error::new(
        ty.span(),
        "unsupported event handler argument type; use BackendRequest, String, Payload, Input<T>, Options<T>, CancelRx, or BroadcastSeq",
    ))
}

fn require_arg_type(method: &ImplItemFn, expected_ident: &str, message: &str) -> Result<()> {
    let Some(FnArg::Typed(arg)) = first_non_receiver_arg(method) else {
        return Ok(());
    };

    let Type::Path(type_path) = arg.ty.as_ref() else {
        return Err(syn::Error::new(arg.ty.span(), message));
    };

    let Some(segment) = type_path.path.segments.last() else {
        return Err(syn::Error::new(type_path.path.span(), message));
    };

    if segment.ident != expected_ident {
        return Err(syn::Error::new(type_path.path.span(), message));
    }

    Ok(())
}

fn is_associated_constructor_candidate(method: &ImplItemFn) -> bool {
    matches!(method.vis, Visibility::Inherited | Visibility::Restricted(_) | Visibility::Public(_))
        && non_receiver_arg_count(method) == method.sig.inputs.len()
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
    let mut has_any_handler = false;

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

        has_any_handler = true;

        if method.sig.asyncness.is_none() {
            return Err(syn::Error::new(
                method.sig.span(),
                "backend handler methods must be async",
            ));
        }

        let event_handler_kind = if !event_args.is_empty() {
            let event_non_receiver_args: Vec<_> = method
                .sig
                .inputs
                .iter()
                .filter(|arg| !matches!(arg, FnArg::Receiver(_)))
                .collect();
            let legacy_event_handler = event_non_receiver_args.len() == 1
                && event_non_receiver_args.first().is_some_and(|arg| is_backend_request_arg(arg));
            if event_non_receiver_args.iter().any(|arg| is_backend_request_arg(arg))
                && !legacy_event_handler
            {
                return Err(syn::Error::new(
                    method.sig.inputs.span(),
                    "event handlers taking BackendRequest must not take additional non-self arguments",
                ));
            }
            if legacy_event_handler {
                EventHandlerKind::Legacy
            } else {
                let extractors = event_non_receiver_args
                    .iter()
                    .map(|arg| event_arg_extractor(arg))
                    .collect::<Result<Vec<_>>>()?;
                if !typed_event_handler_returns_result(method) {
                    return Err(syn::Error::new(
                        method.sig.output.span(),
                        "typed event handlers must return Result<Success, Error>",
                    ));
                }
                EventHandlerKind::Typed { extractors }
            }
        } else {
            EventHandlerKind::Typed { extractors: Vec::new() }
        };

        if !runtime_args.is_empty() {
            let count = non_receiver_arg_count(method);
            if count > 1 {
                return Err(syn::Error::new(
                    method.sig.inputs.span(),
                    "runtime control handlers may take zero or one non-self argument",
                ));
            }
            if count == 1 {
                require_arg_type(
                    method,
                    "RuntimeControlSignal",
                    "runtime control handlers must take RuntimeControlSignal when they accept an argument",
                )?;
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
            event_routes.push(EventRoute {
                pattern,
                method: method_ident.clone(),
                kind: event_handler_kind.clone(),
            });
        }

        if !peer_args.is_empty() {
            let count = non_receiver_arg_count(method);
            if count > 1 {
                return Err(syn::Error::new(
                    method.sig.inputs.span(),
                    "peer control handler may take zero or one non-self argument",
                ));
            }
            if count == 1 {
                require_arg_type(
                    method,
                    "PeerWorkerCommand",
                    "peer control handlers must take PeerWorkerCommand when they accept an argument",
                )?;
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

    if has_any_handler {
        let constructor_candidates: Vec<_> = item_impl
            .items
            .iter()
            .filter_map(|item| {
                let ImplItem::Fn(method) = item else {
                    return None;
                };
                is_associated_constructor_candidate(method).then_some(method.sig.ident.clone())
            })
            .collect();

        match constructor_candidates.as_slice() {
            [ident] if ident == "new" => {}
            [] => {
                return Err(syn::Error::new(
                    item_impl.self_ty.span(),
                    "#[backend_handler] impl blocks must expose exactly one associated constructor named `new`",
                ));
            }
            [ident] => {
                return Err(syn::Error::new(
                    ident.span(),
                    "#[backend_handler] associated constructor must be named `new`",
                ));
            }
            _ => {
                return Err(syn::Error::new(
                    item_impl.self_ty.span(),
                    "#[backend_handler] impl blocks must expose only one associated constructor named `new`",
                ));
            }
        }
    }

    let event_generated = event_routes.iter().enumerate().map(|(idx, route)| {
        let matcher = format_ident!("__backend_handler_match_event_{}", idx);
        let caller = format_ident!("__backend_handler_call_event_{}", idx);
        let pattern = &route.pattern;
        let method = &route.method;
        match &route.kind {
            EventHandlerKind::Legacy => {
                quote! {
                    fn #matcher(route: ::slab_runtime_core::backend::RequestRoute) -> bool {
                        matches!(route, #pattern)
                    }
                    fn #caller<'a>(
                        &'a mut self,
                        req: ::slab_runtime_core::backend::BackendRequest,
                    ) -> ::slab_runtime_core::backend::HandlerFuture<'a> {
                        Box::pin(async move {
                            self.#method(req).await;
                        })
                    }
                }
            }
            EventHandlerKind::Typed { extractors } => {
                let call_args = (0..extractors.len())
                    .map(|arg_idx| format_ident!("__backend_handler_arg_{}_{}", idx, arg_idx))
                    .collect::<Vec<_>>();
                let extraction_stmts = extractors.iter().zip(call_args.iter()).map(
                    |(extractor, binding)| {
                        quote! {
                            let #binding = match #extractor {
                                Ok(value) => value,
                                Err(error) => {
                                    let _ = req
                                        .reply_tx
                                        .send(::slab_runtime_core::backend::BackendReply::error(error));
                                    return;
                                }
                            };
                        }
                    },
                );
                quote! {
                    fn #matcher(route: ::slab_runtime_core::backend::RequestRoute) -> bool {
                        matches!(route, #pattern)
                    }
                    fn #caller<'a>(
                        &'a mut self,
                        req: ::slab_runtime_core::backend::BackendRequest,
                    ) -> ::slab_runtime_core::backend::HandlerFuture<'a> {
                        Box::pin(async move {
                            #(#extraction_stmts)*
                            let __result = self.#method(#(#call_args),*).await;
                            let _ = req.reply_tx.send(
                                ::slab_runtime_core::backend::backend_reply_from_event_result(
                                    __result,
                                ),
                            );
                        })
                    }
                }
            }
        }
    });

    let event_table_entries = (0..event_routes.len()).map(|idx| {
        let matcher = format_ident!("__backend_handler_match_event_{}", idx);
        let caller = format_ident!("__backend_handler_call_event_{}", idx);
        quote! {
            ::slab_runtime_core::backend::RequestRouteMatcher {
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
            fn #matcher(sig: &::slab_runtime_core::backend::RuntimeControlSignal) -> bool {
                matches!(sig, #pattern { .. })
            }
            fn #caller<'a>(
                &'a mut self,
                signal: ::slab_runtime_core::backend::RuntimeControlSignal,
            ) -> ::slab_runtime_core::backend::HandlerFuture<'a> {
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
            ::slab_runtime_core::backend::RuntimeRoute {
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
            fn #matcher(cmd: &::slab_runtime_core::backend::PeerWorkerCommand) -> bool {
                matches!(cmd, #pattern { .. })
            }
            fn #caller<'a>(
                &'a mut self,
                cmd: ::slab_runtime_core::backend::PeerWorkerCommand,
            ) -> ::slab_runtime_core::backend::HandlerFuture<'a> {
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
            ::slab_runtime_core::backend::PeerRoute {
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
                cmd: ::slab_runtime_core::backend::PeerWorkerCommand,
            ) -> ::slab_runtime_core::backend::HandlerFuture<'a> {
                Box::pin(async move {
                    #call
                })
            }
        }
    } else {
        quote! {}
    };

    let peer_fallback_route = if peer_fallback_method.is_some() {
        quote! { Some(Self::__backend_handler_call_peer as ::slab_runtime_core::backend::PeerDispatchFn<Self>) }
    } else {
        quote! { None }
    };

    let lagged_generated = if let Some(method) = &control_lagged_method {
        quote! {
            fn __backend_handler_call_lagged<'a>(
                &'a mut self,
            ) -> ::slab_runtime_core::backend::HandlerFuture<'a> {
                Box::pin(async move {
                    self.#method().await;
                })
            }
        }
    } else {
        quote! {}
    };

    let lagged_route = if control_lagged_method.is_some() {
        quote! { Some(Self::__backend_handler_call_lagged as ::slab_runtime_core::backend::LaggedDispatchFn<Self>) }
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

            pub(crate) fn route_table() -> ::slab_runtime_core::backend::WorkerRouteTable<Self> {
                ::slab_runtime_core::backend::WorkerRouteTable {
                    request_routes: &[#(#event_table_entries),*],
                    runtime_control_routes: &[#(#runtime_table_entries),*],
                    peer_control_routes: &[#(#peer_table_entries),*],
                    peer_control_fallback: #peer_fallback_route,
                    control_lagged_route: #lagged_route,
                }
            }
        }

        #[async_trait::async_trait]
        impl #impl_generics ::slab_runtime_core::backend::RuntimeWorkerHandler for #self_ty #where_clause {
            fn route_table(&self) -> ::slab_runtime_core::backend::WorkerRouteTable<Self> {
                Self::route_table()
            }
        }
    };

    Ok(expanded.into())
}
