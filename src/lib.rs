use proc_macro::TokenStream;
use syn::{ braced, token, parse_macro_input, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::{ quote };

struct Machine {
    name: Ident,
    #[allow(dead_code)]
    brace_token: token::Brace,
    states: Punctuated<StateDefinition, Token![,]>
}

struct StateDefinition {
    name: Ident,
    #[allow(dead_code)]
    brace_token: token::Brace,
    transitions: Punctuated<StateTransition, Token![,]>
}

struct StateTransition {
    event: Ident,
    #[allow(dead_code)]
    separator: Token![=>],
    next_state: Ident
}

impl Parse for Machine {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        Ok(Machine {
            name: input.parse()?,
            brace_token: braced!(content in input),
            states: content.parse_terminated(StateDefinition::parse)?,
        })
    }
}

impl Parse for StateDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        Ok(StateDefinition {
            name: input.parse()?,
            brace_token: braced!(content in input),
            transitions: content.parse_terminated(StateTransition::parse)?,
        })
    }
}

impl Parse for StateTransition {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(StateTransition {
            event: input.parse()?,
            separator: input.parse()?,
            next_state: input.parse()?
        })
    }
}

#[proc_macro]
pub fn statemachine(input: TokenStream) -> TokenStream {
    let machine: Machine = parse_macro_input!(input as Machine);

    let machine_name = &machine.name;
    let state_names = machine.states.iter().map(|x| &x.name);
    let state_names_copy = machine.states.iter().map(|x| &x.name);
    
    let state_structs = machine.states.iter().fold(quote!(), |a, b| {
        let state_name = &b.name;
        let transitions = b.transitions.iter().fold(quote!(), |a, b| {
            // ensure next state is defined as a state
            if let Some(_) = machine.states.iter().find(|x| x.name == b.next_state) {
            } else {
                panic!("undefined state referenced as next state")
            }

            let next_state_name = &b.next_state;
            let event = &b.event;

            quote! {
                #a

                impl From<#machine_name<#state_name>> for #machine_name<#next_state_name> {
                    fn from(val: #machine_name<#state_name>) -> #machine_name<#next_state_name> {
                        #machine_name {
                            state: #next_state_name {
                            }
                        }
                    }
                }

                impl #machine_name<#state_name> {
                    pub fn #event(self) -> State {
                        let next: #machine_name<#next_state_name> = self.into();
                        #machine_name::<#next_state_name>::announce();
                        State::#next_state_name(next)
                    }
                }
            }
        });

        quote! {
            #a

            #[derive(Clone, Copy)]
            pub struct #state_name {}

            impl #machine_name<#state_name> {
                pub fn new() -> Self {
                    #machine_name::<#state_name>::announce();
                    #machine_name {
                        state: #state_name {
                        }
                    }
                }

                fn announce() {
                    println!("the machine currently assumes the {} state", stringify!(#state_name));
                }
            }

            #transitions
        }
    });

    let tokens = quote!{
        #[derive(Clone, Copy)]
        pub struct #machine_name<S> {
            state: S
        }

        #state_structs

        #[derive(Clone, Copy)]
        pub enum State {
            #(#state_names(#machine_name<#state_names>)),*
        }

        pub fn state_from_str(raw_state: &str) -> Option<State> {
            match raw_state {
                #(stringify!(#state_names_copy) => Some(State::#state_names_copy(#machine_name::<#state_names_copy>::new()))),*,
                _ => None
            }
        }
    };

    tokens.into()
}