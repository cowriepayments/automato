use proc_macro::TokenStream;
use syn::{ braced, token, parse_macro_input, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::{ quote, format_ident };

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
    
    let states = machine.states.iter().fold(quote!(), |a, b| {
        let state_name = format_ident!("{}", b.name);
        
        let transitions = b.transitions.iter().fold(quote!(), |a, b| {
            // ensure next state is defined as a state
            if let Some(_) = machine.states.iter().find(|x| x.name == b.next_state) {
            } else {
                panic!("undefined state referenced as next state")
            }

            let next_state_name = format_ident!("{}", b.next_state);
            let event = &b.event;

            quote! {
                #a

                impl From<Machine<#state_name>> for Machine<#next_state_name> {
                    fn from(val: Machine<#state_name>) -> Machine<#next_state_name> {
                        Machine {
                            state: #next_state_name {
                            }
                        }
                    }
                }

                impl Machine<#state_name> {
                    pub fn #event(self) -> Machine<#next_state_name> {
                        let next: Machine<#next_state_name> = self.into();
                        next.announce();
                        next
                    }
                }
            }
        });

        quote! {
            #a

            pub struct #state_name {}

            impl Machine<#state_name> {
                pub fn new() -> Self {
                    Machine {
                        state: #state_name {
                        }
                    }
                }

                fn announce(&self) {
                    println!("the machine currently assumes the {} state", stringify!(#state_name));
                }
            }

            #transitions
        }
    });

    let module_name = &machine.name;
    let tokens = quote!{
        pub mod #module_name {
            #states

            pub struct Machine<S> {
                state: S
            }
        }
    };

    tokens.into()
}