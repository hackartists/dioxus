use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens, TokenStreamExt};
use std::hash;
use syn::parse::Parse;

use crate::location::CallerLocation;

#[derive(Clone, Debug)]
pub struct RawExpr {
    pub expr: TokenStream2,
    pub location: CallerLocation,
}

impl PartialEq for RawExpr {
    fn eq(&self, other: &Self) -> bool {
        self.expr.to_string() == other.expr.to_string()
    }
}

impl Eq for RawExpr {}

impl hash::Hash for RawExpr {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.expr.to_string().hash(state);
    }
}

impl Parse for RawExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        todo!()
    }
}

impl ToTokens for RawExpr {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let exp = &self.expr;

        // Make sure we bind the expression to a variable so the lifetimes are relaxed
        tokens.append_all(quote! {
            {
                let ___nodes = (#exp).into_dyn_node();
                ___nodes
            }
        })
    }
}
