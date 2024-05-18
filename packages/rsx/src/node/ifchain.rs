use super::*;

#[non_exhaustive]
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
pub struct IfChain {
    pub if_token: Token![if],
    pub cond: Box<Expr>,
    pub then_branch: TemplateBody,
    pub else_if_branch: Option<Box<IfChain>>,
    pub else_branch: Option<TemplateBody>,
    pub location: CallerLocation,
}

impl IfChain {
    pub fn for_each_branch(&self, f: &mut impl FnMut(&TemplateBody)) {
        f(&self.then_branch);

        if let Some(else_if) = &self.else_if_branch {
            else_if.for_each_branch(f);
        }

        if let Some(else_branch) = &self.else_branch {
            f(else_branch);
        }
    }

    pub fn to_template_node(&self) -> TemplateNode {
        TemplateNode::Dynamic {
            id: self.location.idx.get(),
        }
    }
}

impl Parse for IfChain {
    fn parse(input: ParseStream) -> Result<Self> {
        let if_token: Token![if] = input.parse()?;

        // stolen from ExprIf
        let cond = Box::new(input.call(Expr::parse_without_eager_brace)?);

        let then_branch = input.parse()?;

        let mut else_branch = None;
        let mut else_if_branch = None;

        // if the next token is `else`, set the else branch as the next if chain
        if input.peek(Token![else]) {
            input.parse::<Token![else]>()?;
            if input.peek(Token![if]) {
                else_if_branch = Some(Box::new(input.parse::<IfChain>()?));
            } else {
                else_branch = Some(input.parse()?);
            }
        }

        Ok(Self {
            cond,
            if_token,
            then_branch,
            else_if_branch,
            else_branch,
            location: CallerLocation::default(),
        })
    }
}

impl ToTokens for IfChain {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let mut body = TokenStream2::new();
        let mut terminated = false;

        let mut elif = Some(self);

        let base_idx = 1123123;

        // let base_idx = self.location.idx.get() * 1000;
        let mut cur_idx = base_idx + 1;

        while let Some(chain) = elif {
            let IfChain {
                if_token,
                cond,
                then_branch,
                else_if_branch,
                else_branch,
                ..
            } = chain;

            body.append_all(quote! { #if_token #cond { {#then_branch} } });

            cur_idx += 1;

            if let Some(next) = else_if_branch {
                body.append_all(quote! { else });
                elif = Some(next);
            } else if let Some(else_branch) = else_branch {
                body.append_all(quote! { else { {#else_branch} } });
                terminated = true;
                break;
            } else {
                elif = None;
            }
        }

        if !terminated {
            body.append_all(quote! {
                else { None }
            });
        }

        tokens.append_all(quote! {
            {
                let ___nodes = (#body).into_dyn_node();
                ___nodes
            }
        })
    }
}
