use proc_macro::TokenStream;
use syn::{ parse_macro_input, braced, token, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::quote;
use std::collections::HashMap;

struct Machine {
    name: Ident,
    shared_data_type: Option<Ident>,
    #[allow(dead_code)]
    brace_token: token::Brace,
    states: Punctuated<StateDefinition, Token![,]>
}

struct StateDefinition {
    init: bool,
    name: Ident,
    associated_data_type: Option<Ident>,
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
        let name: Ident = input.parse()?;
        
        let mut shared_data_type: Option<Ident> = None;
        let colon: Result<Token![:]> = input.parse();
        if let Ok(_) = colon {
            shared_data_type = Some(input.parse()?);
        }

        let content;
        Ok(Machine {
            name,
            shared_data_type,
            brace_token: braced!(content in input),
            states: content.parse_terminated(StateDefinition::parse)?,
        })
    }
}

impl Parse for StateDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut init = false;
        let name: Ident;

        let x: Ident = input.parse()?;
        if x == "init" {
            init = true;
            name = input.parse()?;
        } else {
            name = x;
        }

        let mut associated_data_type: Option<Ident> = None;
        let colon: Result<Token![:]> = input.parse();
        if let Ok(_) = colon {
            associated_data_type = Some(input.parse()?);
        }

        let content;
        Ok(StateDefinition {
            init,
            name,
            associated_data_type,
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
    let m = parse_macro_input!(input as Machine);

    let mut state_data_types = HashMap::new();
    for state in m.states.iter() {
        if let Some(dt) = &state.associated_data_type {
            state_data_types.insert(&state.name, dt);
        }
    }

    let parent_name = &m.name;
    let shared_data_type = &m.shared_data_type;
    
    let state_structs = m.states.iter().map(|x| {
        let state_name = &x.name;
        let data_type = &x.associated_data_type;

        match data_type {
            Some(dt) => quote! {
                struct #state_name {
                    data: #dt
                }

                impl #state_name {
                    fn new(data: #dt) -> Self {
                        Self {
                            data
                        }
                    }

                    fn data(&self) -> &#dt {
                        &self.data
                    }
                }
            },
            None => quote! {
                struct #state_name {}

                impl #state_name {
                    fn new() -> Self {
                        Self {}
                    }
                }
            }
        }
    });

    let transitions_block = m.states.iter().fold(quote!(), |acc, x| {
        let state_name = &x.name;
        let transitions = x.transitions.iter().map(|y| {
            let event = &y.event;
            let arg = state_data_types.get(state_name);
            let next_state_name = &y.next_state;
            
            match arg {
                Some(a) => match shared_data_type {
                    Some(_) => quote! {
                        impl #parent_name<#state_name> {
                            fn #event(self, data: #a) -> #parent_name<#next_state_name> {
                                #parent_name::<#next_state_name>::new(#next_state_name::new(data), self.data)
                            }
                        }
                    },
                    None => quote! {
                        impl #parent_name<#state_name> {
                            fn #event(self, data: #a) -> #parent_name<#next_state_name> {
                                #parent_name::<#next_state_name>::new(#next_state_name::new(data))
                            }
                        }
                    }
                },
                None => match shared_data_type {
                    Some(_) => quote! {
                        impl #parent_name<#state_name> {
                            fn #event(self) -> #parent_name<#next_state_name> {
                                #parent_name::<#next_state_name>::new(#next_state_name::new(), self.data)
                            }
                        }
                    },
                    None => quote! {
                        impl #parent_name<#state_name> {
                            fn #event(self) -> #parent_name<#next_state_name> {
                                #parent_name::<#next_state_name>::new(#next_state_name::new())
                            }
                        }
                    }
                }
            }
        });

        quote! {
            #acc

            #(#transitions)*
        }
    });

    let parent_state_impls = m.states.iter().map(|x| {
        let state_name = &x.name;
        match shared_data_type {
            Some(sdt) => {
                let constructor = quote! {
                    impl #parent_name<#state_name> {
                        fn new(state: #state_name, data: #sdt) -> Self {
                            Self {
                                state,
                                data
                            }
                        }
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                fn init(data: #sdt, state_data: #dt) -> Self {
                                    Self::new(#state_name::new(state_data), data)
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                fn init(data: #sdt) -> Self {
                                    Self::new(#state_name::new(), data)
                                }
                            }
                        }
                    }
                }
            },
            None => {
                let constructor = quote! {
                    impl #parent_name<#state_name> {
                        fn new(state: #state_name) -> Self {
                            Self {
                                state
                            }
                        }
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                fn init(state_data: #dt) -> Self {
                                    Self::new(#state_name::new(state_data))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                fn init() -> Self {
                                    Self::new(#state_name::new())
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let parent_struct = match shared_data_type {
        Some(sdt) => quote! {
            struct #parent_name<T> {
                state: T,
                data: #sdt
            }
        },
        None => quote! {
            struct #parent_name<T> {
                state: T
            }
        }
    };

    let out = quote! {
        #parent_struct
        #(#state_structs)*
        #(#parent_state_impls)*
        #transitions_block
    };

    out.into()
}