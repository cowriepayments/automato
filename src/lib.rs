use proc_macro::TokenStream;
use syn::{ braced, token, parse_macro_input, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::quote;

struct Machine {
    #[allow(dead_code)]
    brace_token: token::Brace,
    states: Punctuated<StateDefinition, Token![,]>
}

struct StateDefinition {
    name: Ident,
    #[allow(dead_code)]
    init: Option<Ident>,
    init_data_type: Option<Ident>,
    #[allow(dead_code)]
    brace_token: token::Brace,
    transitions: Punctuated<StateTransition, Token![,]>
}

struct StateTransition {
    event: Ident,
    data_type: Option<Ident>,
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
            init: match input.parse() {
                Ok(r) => {
                    assert_eq!(r, "init");
                    Some(r)
                },
                Err(_) => None
            },
            init_data_type: match input.parse() {
                Ok(r) => Some(r),
                Err(_) => None
            },
            brace_token: braced!(content in input),
            transitions: content.parse_terminated(StateTransition::parse)?,
        })
    }
}

impl Parse for StateTransition {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(StateTransition {
            event: input.parse()?,
            data_type: match input.parse() {
                Ok(r) => Some(r),
                Err(_) => None
            },
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

            let event_impl_block = match &b.data_type {
                Some(data_type) => quote! {
                    impl<T: Listener> Machine<#state_name, T> {
                        pub fn #event(self, data: #data_type) -> Result<State<T>, ()> {
                            let next: Machine<#next_state_name, T> = self.into();
                            match next.t.on_transition(stringify!(#state_name), stringify!(#next_state_name), Some(data)) {
                                Ok(_) => Ok(State::#next_state_name(next)),
                                Err(_) => Err(())
                            }
                        }
                    }
                },
                None => quote! {
                    impl<T: Listener> Machine<#state_name, T> {
                        pub fn #event(self) -> Result<State<T>, ()> {
                            let next: Machine<#next_state_name, T> = self.into();
                            match next.t.on_transition(stringify!(#state_name), stringify!(#next_state_name), Option::<()>::None) {
                                Ok(_) => Ok(State::#next_state_name(next)),
                                Err(_) => Err(())
                            }
                        }
                    }
                }
            };

            quote! {
                #a

                impl<T: Listener> From<Machine<#state_name, T>> for Machine<#next_state_name, T> {
                    fn from(val: Machine<#state_name, T>) -> Machine<#next_state_name, T> {
                        Machine {
                            state: #next_state_name {
                            },
                            t: val.t
                        }
                    }
                }

                #event_impl_block
            }
        });

        let init_impl_block = match b.init {
            Some(_) => {
                match &b.init_data_type {
                    Some(data_type) => {
                        quote! {
                            impl<T: Listener> Machine<#state_name, T> {
                                pub fn init(t: T, data: #data_type) -> Result<Self, ()> {
                                    match t.on_init(stringify!(#state_name), Some(data)) {
                                        Ok(_) => Ok(Self::new(t)),
                                        Err(_) => Err(())
                                    }
                                }
                            }
                        }
                    },
                    None => quote! {
                        impl<T: Listener> Machine<#state_name, T> {
                            pub fn init(t: T) -> Result<Self, ()> {
                                match t.on_init(stringify!(#state_name), Option::<()>::None) {
                                    Ok(_) => Ok(Self::new(t)),
                                    Err(_) => Err(())
                                }
                            }
                        }
                    }
                }
            },
            None => quote!()
        };

        quote! {
            #a

            struct #state_name {}

            impl<T: Listener> Machine<#state_name, T> {
                fn new(t: T) -> Self {
                    Machine {
                        state: #state_name {
                        },
                        t
                    }
                }
            }

            #init_impl_block

            #transitions
        }
    });

    let tokens = quote!{
        pub trait Listener {
            fn on_init<T: Serialize>(&self, to: &str, data: Option<T>) -> Result<(), ()>;
            fn on_transition<T: Serialize>(&self, from: &str, to: &str, data: Option<T>) -> Result<(), ()>;
        }

        pub struct Machine<S, T: Listener> {
            state: S,
            t: T
        }

        #state_structs

        pub enum State<T: Listener> {
            #(#state_names(Machine<#state_names, T>)),*
        }

        pub fn state_from_str<T: Listener>(raw_state: &str, t: T) -> Option<State<T>> {
            match raw_state {
                #(stringify!(#state_names_copy) => Some(State::#state_names_copy(Machine::<#state_names_copy, T>::new(t)))),*,
                _ => None
            }
        }
    };

    tokens.into()
}