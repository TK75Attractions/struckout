use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Generates shared accessors so callers can inspect fields without repeating matches.
///
/// Each `as_<variant>` method returns a tuple of shared field references in
/// declaration order, or the actual variant's name. Methods have the enum's visibility.
/// A single field returns a one-element tuple; a unit variant returns `()`.
/// Both accessor macros can be derived on the same enum.
///
/// ```
/// use struckout_macros::{EnumAsInner, EnumAsInnerMut};
///
/// #[derive(EnumAsInner, EnumAsInnerMut)]
/// enum Session {
///     ConfiguringDifficulty { player: String, difficulty: Option<u32> },
///     Running(u32),
///     Idle,
/// }
///
/// let mut session = Session::ConfiguringDifficulty {
///     player: "Alice".into(), difficulty: Some(3),
/// };
/// let shared: &Session = &session;
/// let (player, difficulty): (&String, &Option<u32>) =
///     shared.as_configuring_difficulty().expect("configuring");
/// assert_eq!(player, "Alice");
/// assert_eq!(*difficulty, Some(3));
/// assert_eq!(shared.as_running(), Err("ConfiguringDifficulty"));
/// assert_eq!(shared.as_idle(), Err("ConfiguringDifficulty"));
///
/// let (_, difficulty) = session.as_configuring_difficulty_mut().expect("configuring");
/// *difficulty = Some(4);
/// assert_eq!(session.as_configuring_difficulty().map(|(_, difficulty)| *difficulty), Ok(Some(4)));
///
/// let session = Session::Running(7);
/// let (score,): (&u32,) = session.as_running().expect("running");
/// assert_eq!(*score, 7);
/// assert_eq!(session.as_configuring_difficulty(), Err("Running"));
/// let session = Session::Idle;
/// assert_eq!(session.as_idle(), Ok(()));
/// assert_eq!(session.as_running(), Err("Idle"));
/// ```
///
/// Generics, where clauses, raw identifiers, and conditional variants are preserved.
///
/// ```
/// use struckout_macros::EnumAsInner;
///
/// #[derive(EnumAsInner)]
/// pub enum Value<'a, T, const N: usize> where T: Copy {
///     HTTPResponse { r#type: &'a str, values: [T; N] },
///     Empty {},
///     EmptyTuple(),
///     #[cfg(any())]
///     Disabled(UndefinedType),
/// }
///
/// let value = Value::HTTPResponse { r#type: "text", values: [1; 2] };
/// let (kind, values): (&&str, &[u32; 2]) = value.as_http_response().expect("response");
/// assert_eq!(*kind, "text");
/// assert_eq!(*values, [1, 1]);
/// let value: Value<'_, u32, 2> = Value::Empty {};
/// assert_eq!(value.as_empty(), Ok(()));
/// let value: Value<'_, u32, 2> = Value::EmptyTuple();
/// assert_eq!(value.as_empty_tuple(), Ok(()));
/// ```
///
/// Only enums can derive this macro.
///
/// ```compile_fail
/// #[derive(struckout_macros::EnumAsInner)]
/// struct NotAnEnum;
/// ```
#[proc_macro_derive(EnumAsInner)]
pub fn derive_enum_as_inner(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_enum_as_inner(&input, ReferenceKind::Shared)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates mutable accessors so callers can update fields without repeating matches.
///
/// Each `as_<variant>_mut` method returns a tuple of mutable field references in
/// declaration order, or the actual variant's name. Methods have the enum's visibility.
/// A single field returns a one-element tuple; a unit variant returns `()`.
///
/// ```
/// use struckout_macros::EnumAsInnerMut;
///
/// #[derive(EnumAsInnerMut)]
/// enum Session {
///     ConfiguringDifficulty { player: String, difficulty: Option<u32> },
///     Running(u32),
///     Idle,
/// }
///
/// let mut session = Session::ConfiguringDifficulty {
///     player: "Alice".into(), difficulty: None,
/// };
/// let (player, difficulty) = session.as_configuring_difficulty_mut().expect("configuring");
/// player.push('!');
/// *difficulty = Some(3);
/// let (player, difficulty) = session.as_configuring_difficulty_mut().expect("configuring");
/// assert_eq!(player, "Alice!");
/// assert_eq!(*difficulty, Some(3));
/// assert_eq!(session.as_running_mut(), Err("ConfiguringDifficulty"));
/// assert_eq!(session.as_idle_mut(), Err("ConfiguringDifficulty"));
///
/// session = Session::Running(1);
/// let (score,) = session.as_running_mut().expect("running");
/// *score += 2;
/// assert_eq!(session.as_running_mut().map(|(score,)| *score), Ok(3));
/// assert_eq!(session.as_configuring_difficulty_mut(), Err("Running"));
///
/// session = Session::Idle;
/// assert_eq!(session.as_idle_mut(), Ok(()));
/// assert_eq!(session.as_running_mut(), Err("Idle"));
/// ```
///
/// Generics, where clauses, raw identifiers, and conditional variants are preserved.
///
/// ```
/// use struckout_macros::EnumAsInnerMut;
///
/// #[derive(EnumAsInnerMut)]
/// pub enum Value<'a, T, const N: usize> where T: Copy {
///     HTTPResponse { r#type: &'a str, values: [T; N] },
///     Single(T),
///     Empty {},
///     EmptyTuple(),
///     #[cfg(any())]
///     Disabled(UndefinedType),
/// }
///
/// let mut value = Value::HTTPResponse { r#type: "old", values: [1; 2] };
/// let (kind, values) = value.as_http_response_mut().expect("response");
/// *kind = "new";
/// values[0] = 2;
/// assert_eq!(value.as_http_response_mut().map(|(kind, values)| (*kind, *values)),
///            Ok(("new", [2, 1])));
/// value = Value::Single(3);
/// assert_eq!(value.as_single_mut().map(|(item,)| *item), Ok(3));
/// value = Value::Empty {};
/// assert_eq!(value.as_empty_mut(), Ok(()));
/// value = Value::EmptyTuple();
/// assert_eq!(value.as_empty_tuple_mut(), Ok(()));
/// ```
///
/// Only enums can derive this macro.
///
/// ```compile_fail
/// #[derive(struckout_macros::EnumAsInnerMut)]
/// struct NotAnEnum;
/// ```
#[proc_macro_derive(EnumAsInnerMut)]
pub fn derive_enum_as_inner_mut(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_enum_as_inner(&input, ReferenceKind::Mutable)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

enum ReferenceKind {
    Shared,
    Mutable,
}

fn expand_enum_as_inner(input: &DeriveInput, kind: ReferenceKind) -> syn::Result<TokenStream2> {
    let (macro_name, suffix, mutability, reference_description) = match kind {
        ReferenceKind::Shared => ("EnumAsInner", "", quote! {}, "shared"),
        ReferenceKind::Mutable => ("EnumAsInnerMut", "_mut", quote! { mut }, "mutable"),
    };
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            format!("{macro_name} は enum にのみ使用できます"),
        ));
    };

    let name = &input.ident;
    let visibility = &input.vis;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let methods = data.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let method = format_ident!("as_{}{suffix}", variant_name.unraw().to_string().to_snake_case());
        let types = variant.fields.iter().map(|field| &field.ty);
        let bindings: Vec<_> = (0..variant.fields.len())
            .map(|index| format_ident!("field_{index}"))
            .collect();
        let pattern = match &variant.fields {
            Fields::Named(fields) => {
                let names = fields.named.iter().map(|field| &field.ident);
                quote! { { #(#names: #bindings),* } }
            }
            Fields::Unnamed(_) => quote! { (#(#bindings),*) },
            Fields::Unit => quote! {},
        };
        let cfg = variant.attrs.iter().filter(|attr| attr.path().is_ident("cfg"));
        let errors = data.variants.iter().filter(|other| other.ident != *variant_name).map(|other| {
            let other_name = &other.ident;
            let actual = other_name.unraw().to_string();
            let cfg = other.attrs.iter().filter(|attr| attr.path().is_ident("cfg"));
            let pattern = match &other.fields {
                Fields::Named(_) => quote! { { .. } },
                Fields::Unnamed(_) => quote! { (..) },
                Fields::Unit => quote! {},
            };
            quote! {
                #(#cfg)*
                Self::#other_name #pattern => ::core::result::Result::Err(#actual),
            }
        });
        let doc = format!(
            "Returns {reference_description} references to the fields of [`{name}::{variant_name}`] if `self` \
             is that variant, otherwise `Err(actual_state)`, so callers can report the actual state."
        );

        quote! {
            #(#cfg)*
            #[doc = #doc]
            #visibility fn #method(&#mutability self) -> ::core::result::Result<
                (#(&#mutability #types,)*),
                &'static str,
            > {
                match self {
                    Self::#variant_name #pattern => ::core::result::Result::Ok((#(#bindings,)*)),
                    #(#errors)*
                }
            }
        }
    });

    Ok(quote! {
        impl #impl_generics #name #type_generics #where_clause {
            #(#methods)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_structs_and_unions() {
        for input in [
            quote! { struct NotAnEnum; },
            quote! { union NotAnEnum { field: u32 } },
        ] {
            let input = syn::parse2(input).expect("valid derive input");
            for (kind, macro_name) in [
                (ReferenceKind::Shared, "EnumAsInner"),
                (ReferenceKind::Mutable, "EnumAsInnerMut"),
            ] {
                let error =
                    expand_enum_as_inner(&input, kind).expect_err("only enums are supported");
                assert_eq!(
                    error.to_string(),
                    format!("{macro_name} は enum にのみ使用できます")
                );
            }
        }
    }

    #[test]
    fn supports_empty_enums() {
        let input = syn::parse_quote! { enum Empty {} };
        for kind in [ReferenceKind::Shared, ReferenceKind::Mutable] {
            let output = expand_enum_as_inner(&input, kind).expect("empty enums are supported");
            assert_eq!(output.to_string(), quote! { impl Empty {} }.to_string());
        }
    }
}
