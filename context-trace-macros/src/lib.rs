use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{
    quote,
    ToTokens,
};
use syn::{
    parse_macro_input,
    DeriveInput,
    ItemFn,
};

#[proc_macro_attribute]
pub fn instrument_sig(
    args: TokenStream,
    input: TokenStream,
) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);

    // Convert args to string to check for fields and level
    let args_str = args.to_string();
    let has_fields = args_str.contains("fields");
    let has_level = args_str.contains("level");

    // Extract the function signature as a string
    let fn_sig = extract_function_signature(&input_fn);

    // Auto-detect trait implementation context
    let trait_context = detect_trait_context(&input_fn);

    // Parse args as TokenStream2 for manipulation
    let args: TokenStream2 = args.into();

    // Build the new attribute arguments with fn_sig and trait context added to or merged with fields
    let new_args = if has_fields {
        // Need to merge with existing fields
        merge_fields_into_args(args, &fn_sig, &trait_context)
    } else {
        // No existing fields, create fields() with fn_sig and trait context
        let mut field_content = quote! { fn_sig = #fn_sig };

        // Add auto-detected trait context if not manually specified
        if trait_context.has_self {
            field_content
                .extend(quote! { , self_type = std::any::type_name::<Self>() });
        }

        for assoc_type in &trait_context.associated_types {
            let type_field_name = format!("{}_type", assoc_type.to_lowercase());
            let type_field_ident = syn::Ident::new(
                &type_field_name,
                proc_macro2::Span::call_site(),
            );
            let type_ident =
                syn::Ident::new(assoc_type, proc_macro2::Span::call_site());
            field_content.extend(quote! { , #type_field_ident = std::any::type_name::<Self::#type_ident>() });
        }

        if args.is_empty() {
            quote! { fields(#field_content) }
        } else {
            quote! { #args, fields(#field_content) }
        }
    };

    // Add default level = "debug" if no level was specified
    let final_args = if has_level {
        new_args
    } else {
        quote! { level = "debug", #new_args }
    };

    // Generate the output with #[instrument] attribute
    let output = quote! {
        #[tracing::instrument(#final_args)]
        #input_fn
    };

    output.into()
}

/// Information about potential trait implementation context
struct TraitContext {
    has_self: bool,
    associated_types: Vec<String>,
}

/// Detect if this function is likely a trait implementation and extract context
fn detect_trait_context(func: &ItemFn) -> TraitContext {
    let mut has_self = false;
    let mut associated_types = Vec::new();

    // Check if function has self parameter
    for input in &func.sig.inputs {
        if let syn::FnArg::Receiver(_) = input {
            has_self = true;
            break;
        }
    }

    // Check return type for associated types (Self::TypeName pattern)
    if let syn::ReturnType::Type(_, ty) = &func.sig.output {
        extract_associated_types(ty, &mut associated_types);
    }

    TraitContext {
        has_self,
        associated_types,
    }
}

/// Recursively extract associated type names from a type (e.g., Self::Next, Result<Self::Next, Self>)
fn extract_associated_types(
    ty: &syn::Type,
    result: &mut Vec<String>,
) {
    match ty {
        syn::Type::Path(type_path) => {
            // Look for Self::AssocType pattern in the path itself
            let segments: Vec<_> = type_path.path.segments.iter().collect();
            for i in 0..segments.len().saturating_sub(1) {
                if segments[i].ident == "Self" {
                    // Found Self, next segment is the associated type
                    let assoc_name = segments[i + 1].ident.to_string();
                    if !result.contains(&assoc_name) {
                        result.push(assoc_name);
                    }
                }
            }

            // Also check for qualified self (e.g., <Self as Trait>::Type)
            if let Some(qself) = &type_path.qself {
                if let syn::Type::Path(self_path) = &*qself.ty {
                    if self_path.path.segments.len() == 1
                        && self_path.path.segments[0].ident == "Self"
                    {
                        // This is Self::Something
                        if let Some(segment) =
                            type_path.path.segments.get(qself.position)
                        {
                            let assoc_name = segment.ident.to_string();
                            if !result.contains(&assoc_name) {
                                result.push(assoc_name);
                            }
                        }
                    }
                }
            }

            // Also check generic arguments for nested associated types
            for segment in &type_path.path.segments {
                if let syn::PathArguments::AngleBracketed(args) =
                    &segment.arguments
                {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner_ty) = arg {
                            extract_associated_types(inner_ty, result);
                        }
                    }
                }
            }
        },
        syn::Type::Tuple(tuple) =>
            for elem in &tuple.elems {
                extract_associated_types(elem, result);
            },
        syn::Type::Array(array) => {
            extract_associated_types(&array.elem, result);
        },
        syn::Type::Ptr(ptr) => {
            extract_associated_types(&ptr.elem, result);
        },
        syn::Type::Reference(reference) => {
            extract_associated_types(&reference.elem, result);
        },
        syn::Type::Slice(slice) => {
            extract_associated_types(&slice.elem, result);
        },
        syn::Type::Paren(paren) => {
            extract_associated_types(&paren.elem, result);
        },
        syn::Type::Group(group) => {
            extract_associated_types(&group.elem, result);
        },
        _ => {},
    }
}

/// Merge fn_sig and trait context into existing fields() argument
fn merge_fields_into_args(
    args: TokenStream2,
    fn_sig: &str,
    trait_context: &TraitContext,
) -> TokenStream2 {
    use proc_macro2::{
        Delimiter,
        Group,
        TokenTree,
    };

    let mut result = Vec::new();
    let mut iter = args.into_iter().peekable();

    while let Some(token) = iter.next() {
        match &token {
            TokenTree::Ident(ident) if *ident == "fields" => {
                // Found fields, add it to result
                result.push(token.clone());

                // Next should be the parenthesized group
                if let Some(TokenTree::Group(group)) = iter.next() {
                    if group.delimiter() == Delimiter::Parenthesis {
                        // Create new group with fn_sig and trait context prepended
                        let mut new_stream = TokenStream2::new();

                        // Add fn_sig = "..."
                        new_stream.extend(quote! { fn_sig = #fn_sig });

                        // Check if user already specified self_type or associated types
                        let group_str = group.stream().to_string();
                        let has_user_self_type =
                            group_str.contains("self_type");

                        // Add auto-detected self_type if not manually specified
                        if trait_context.has_self && !has_user_self_type {
                            new_stream.extend(quote! { , self_type = std::any::type_name::<Self>() });
                        }

                        // Add auto-detected associated types if not manually specified
                        for assoc_type in &trait_context.associated_types {
                            let type_field_name =
                                format!("{}_type", assoc_type.to_lowercase());
                            if !group_str.contains(&type_field_name) {
                                let type_field_ident = syn::Ident::new(
                                    &type_field_name,
                                    proc_macro2::Span::call_site(),
                                );
                                let type_ident = syn::Ident::new(
                                    assoc_type,
                                    proc_macro2::Span::call_site(),
                                );
                                new_stream.extend(quote! { , #type_field_ident = std::any::type_name::<Self::#type_ident>() });
                            }
                        }

                        // Add comma if group is not empty
                        if !group.stream().is_empty() {
                            new_stream.extend(quote! { , });
                            new_stream.extend(group.stream());
                        }

                        // Create new group
                        let new_group =
                            Group::new(Delimiter::Parenthesis, new_stream);
                        result.push(TokenTree::Group(new_group));
                    } else {
                        result.push(TokenTree::Group(group));
                    }
                }
            },
            _ => result.push(token),
        }
    }

    result.into_iter().collect()
}

/// An attribute macro that wraps `#[instrument]` and automatically adds the function signature.
///
/// This macro extracts the function signature at compile time and adds it as a `fn_sig` field
/// to the tracing span, then delegates to the standard `#[instrument]` macro.
///
/// # Usage
///
/// ```rust,ignore
/// use context_trace_macros::instrument_sig;
///
/// #[instrument_sig]
/// fn my_function(x: i32, y: &str) -> Result<bool, Error> {
///     // function body
/// }
///
/// // With additional instrument parameters:
/// #[instrument_sig(skip(y), level = "debug")]
/// fn my_function(x: i32, y: &str) -> Result<bool, Error> {
///     // function body
/// }
/// ```
///
/// The generated code will include `fn_sig = "fn my_function(x: i32, y: &str) -> Result<bool, Error>"`
/// as a field in the span.
/// Extract the function signature as a readable string
fn extract_function_signature(func: &ItemFn) -> String {
    let sig = &func.sig;
    let fn_name = &sig.ident;

    // Build parameter list - normalize whitespace aggressively
    let mut params = Vec::new();
    for input in &sig.inputs {
        let param_str = quote!(#input).to_string();
        params.push(normalize_spacing(&param_str));
    }
    let params_str = params.join(", ");

    // Build return type - normalize whitespace
    let return_type = match &sig.output {
        syn::ReturnType::Default => String::new(),
        syn::ReturnType::Type(_, ty) => {
            let ty_str = quote!(#ty).to_string();
            format!(" -> {}", normalize_spacing(&ty_str))
        },
    };

    // Handle async
    let async_str = if sig.asyncness.is_some() {
        "async "
    } else {
        ""
    };

    // Handle const
    let const_str = if sig.constness.is_some() {
        "const "
    } else {
        ""
    };

    // Handle unsafe
    let unsafe_str = if sig.unsafety.is_some() {
        "unsafe "
    } else {
        ""
    };

    format!(
        "{}{}{}fn {}({}){}",
        const_str, async_str, unsafe_str, fn_name, params_str, return_type
    )
}

/// Normalize spacing in type signatures - remove extra whitespace around punctuation
fn normalize_spacing(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        // Remove spaces around operators
        .replace("& ", "&")
        .replace("* ", "*")
        .replace(" &", "&")
        .replace(" *", "*")
        // Handle colons
        .replace(" :", ":")
        .replace(" ::", "::")
        .replace(":: ", "::")
        // Handle angle brackets
        .replace(" <", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace("> ", ">")
        // Handle commas
        .replace(" ,", ",")
}

/// Attribute macro for trait implementations that automatically adds trait context to all methods
///
/// This macro can be applied to an `impl Trait for Type` block and will automatically add
/// `trait_name` to all instrumented methods, avoiding repetition.
///
/// # Usage
///
/// ```rust,ignore
/// #[instrument_trait_impl]
/// impl StateAdvance for ParentState {
///     type Next = RootChildState;
///     
///     #[instrument_sig(skip(self, trav))]
///     fn advance_state<G: HasGraph>(self, trav: &G) -> Result<Self::Next, Self> {
///         // Method implementation
///         // trait_name = "StateAdvance" is automatically added
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn instrument_trait_impl(
    _args: TokenStream,
    input: TokenStream,
) -> TokenStream {
    let input_impl = parse_macro_input!(input as syn::ItemImpl);

    // Extract the trait name from the impl block
    let trait_name = if let Some((_, path, _)) = &input_impl.trait_ {
        // Get the last segment of the trait path (e.g., StateAdvance from context::StateAdvance)
        path.segments.last().map(|seg| seg.ident.to_string())
    } else {
        None
    };

    // If this is not a trait impl, just return the input unchanged
    let Some(trait_name_str) = trait_name else {
        return input_impl.into_token_stream().into();
    };

    // Process each item in the impl block
    let mut new_items = Vec::new();

    for item in input_impl.items {
        match item {
            syn::ImplItem::Fn(mut method) => {
                // Check if the method already has instrument_sig attribute
                let has_instrument_sig = method.attrs.iter().any(|attr| {
                    attr.path()
                        .segments
                        .last()
                        .map(|seg| seg.ident == "instrument_sig")
                        .unwrap_or(false)
                });

                if has_instrument_sig {
                    // Modify the instrument_sig attribute to add trait_name if not already present
                    for attr in &mut method.attrs {
                        if attr
                            .path()
                            .segments
                            .last()
                            .map(|seg| seg.ident == "instrument_sig")
                            .unwrap_or(false)
                        {
                            // Parse the attribute arguments
                            let attr_str = quote!(#attr).to_string();

                            // Check if trait_name is already specified
                            if !attr_str.contains("trait_name") {
                                // Inject trait_name into the fields
                                let new_attr = inject_trait_name_into_attr(
                                    attr,
                                    &trait_name_str,
                                );
                                *attr = new_attr;
                            }
                        }
                    }
                }

                new_items.push(syn::ImplItem::Fn(method));
            },
            other => new_items.push(other),
        }
    }

    // Reconstruct the impl block with modified items
    let output = syn::ItemImpl {
        items: new_items,
        ..input_impl
    };

    quote!(#output).into()
}

/// Inject trait_name into an instrument_sig attribute
fn inject_trait_name_into_attr(
    attr: &syn::Attribute,
    trait_name: &str,
) -> syn::Attribute {
    use syn::Meta;

    // Try to parse the attribute as a meta list
    if let Meta::List(mut meta_list) = attr.meta.clone() {
        // Convert the existing tokens to string to manipulate
        let tokens_str = meta_list.tokens.to_string();

        // Check if there's already a fields() section
        let new_tokens = if tokens_str.contains("fields") {
            // Add trait_name to existing fields
            let modified = tokens_str.replace(
                "fields(",
                &format!("fields(trait_name = \"{}\" , ", trait_name),
            );
            modified.parse::<TokenStream2>().unwrap()
        } else {
            // Add fields() with trait_name
            let addition = format!(", fields(trait_name = \"{}\")", trait_name);
            let mut tokens = meta_list.tokens.clone();
            tokens.extend(addition.parse::<TokenStream2>().unwrap());
            tokens
        };

        meta_list.tokens = new_tokens;

        syn::Attribute {
            meta: Meta::List(meta_list),
            ..attr.clone()
        }
    } else {
        // If parsing fails, return original
        attr.clone()
    }
}

/// Derive macro that generates a Debug implementation including the full type path.
///
/// This is useful for tracing/logging where you want to see the full module path
/// of a type (e.g., `context_search::response::MatchResult`) rather than just
/// the short name (`MatchResult`).
///
/// # Example
///
/// ```ignore
/// use context_trace_macros::TypedDebug;
///
/// #[derive(TypedDebug)]
/// struct MyResult {
///     value: i32,
///     name: String,
/// }
///
/// let r = MyResult { value: 42, name: "test".into() };
/// // Output: "my_crate::MyResult { value: 42, name: \"test\" }"
/// println!("{:?}", r);
/// ```
#[proc_macro_derive(TypedDebug)]
pub fn typed_debug_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let debug_body = generate_debug_body(&input);

    let expanded = quote! {
        impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // First write the full type path
                let type_name = std::any::type_name::<Self>();
                #debug_body
            }
        }
    };

    expanded.into()
}

/// Generate the debug body based on the data structure
fn generate_debug_body(input: &DeriveInput) -> TokenStream2 {
    match &input.data {
        syn::Data::Struct(data) => generate_struct_debug(&input.ident, data),
        syn::Data::Enum(data) => generate_enum_debug(&input.ident, data),
        syn::Data::Union(_) => {
            quote! {
                write!(f, "{}", type_name)
            }
        }
    }
}

/// Generate debug implementation for a struct
fn generate_struct_debug(name: &syn::Ident, data: &syn::DataStruct) -> TokenStream2 {
    match &data.fields {
        syn::Fields::Named(fields) => {
            let field_names: Vec<_> = fields.named.iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();
            let field_strs: Vec<_> = field_names.iter()
                .map(|n| n.to_string())
                .collect();

            quote! {
                f.debug_struct(type_name)
                    #(.field(#field_strs, &self.#field_names))*
                    .finish()
            }
        }
        syn::Fields::Unnamed(fields) => {
            let indices: Vec<_> = (0..fields.unnamed.len())
                .map(syn::Index::from)
                .collect();

            quote! {
                f.debug_tuple(type_name)
                    #(.field(&self.#indices))*
                    .finish()
            }
        }
        syn::Fields::Unit => {
            let _ = name;
            quote! {
                write!(f, "{}", type_name)
            }
        }
    }
}

/// Generate debug implementation for an enum
fn generate_enum_debug(name: &syn::Ident, data: &syn::DataEnum) -> TokenStream2 {
    let _ = name;
    let variants: Vec<_> = data.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let variant_str = variant_name.to_string();

        match &variant.fields {
            syn::Fields::Named(fields) => {
                let field_names: Vec<_> = fields.named.iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let field_strs: Vec<_> = field_names.iter()
                    .map(|n| n.to_string())
                    .collect();

                quote! {
                    Self::#variant_name { #(#field_names),* } => {
                        let variant_type = format!("{}::{}", type_name, #variant_str);
                        f.debug_struct(&variant_type)
                            #(.field(#field_strs, #field_names))*
                            .finish()
                    }
                }
            }
            syn::Fields::Unnamed(fields) => {
                let bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("__field{}", i), proc_macro2::Span::call_site()))
                    .collect();

                quote! {
                    Self::#variant_name(#(#bindings),*) => {
                        let variant_type = format!("{}::{}", type_name, #variant_str);
                        f.debug_tuple(&variant_type)
                            #(.field(#bindings))*
                            .finish()
                    }
                }
            }
            syn::Fields::Unit => {
                quote! {
                    Self::#variant_name => {
                        write!(f, "{}::{}", type_name, #variant_str)
                    }
                }
            }
        }
    }).collect();

    quote! {
        match self {
            #(#variants)*
        }
    }
}
