use proc_macro::TokenStream;
use syn::{ braced, token, parse_macro_input, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::{ quote };

struct Machine {
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

                impl<T: OnChangeState> From<Machine<#state_name, T>> for Machine<#next_state_name, T> {
                    fn from(val: Machine<#state_name, T>) -> Machine<#next_state_name, T> {
                        Machine {
                            state: #next_state_name {
                            },
                            t: val.t
                        }
                    }
                }

                impl<T: OnChangeState> Machine<#state_name, T> {
                    pub fn #event(self) -> Result<State<T>, ()> {
                        let next: Machine<#next_state_name, T> = self.into();
                        next.t.on_change_state(stringify!(#state_name), stringify!(#next_state_name)).unwrap();
                        State::#next_state_name(next)
                    }
                }
            }
        });

        quote! {
            #a

            // #[derive(Clone, Copy)]
            pub struct #state_name {}

            impl<T: OnChangeState> Machine<#state_name, T> {
                pub fn new(t: T) -> Self {
                    Machine {
                        state: #state_name {
                        },
                        t
                    }
                }
            }

            #transitions
        }
    });

    let tokens = quote!{
        pub trait OnChangeState {
            fn on_change_state(&self, from: &str, to: &str) -> Result<(), ()>;
        }

        // #[derive(Clone, Copy)]
        pub struct Machine<S, T: OnChangeState> {
            state: S,
            t: T
        }

        #state_structs

        // #[derive(Clone, Copy)]
        pub enum State<T: OnChangeState> {
            #(#state_names(Machine<#state_names, T>)),*
        }

        pub fn state_from_str<T: OnChangeState>(raw_state: &str, t: T) -> Option<State<T>> {
            match raw_state {
                #(stringify!(#state_names_copy) => Some(State::#state_names_copy(Machine::<#state_names_copy, T>::new(t)))),*,
                _ => None
            }
        }
    };

    tokens.into()
}