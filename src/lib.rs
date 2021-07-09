use proc_macro::TokenStream;
use syn::{ parse_macro_input, braced, token, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::{quote, format_ident};
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

    let state_names = m.states.iter().map(|x| &x.name);

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
                        impl<T: Observer> #parent_name<#state_name, T> {
                            fn #event(self, data: #a) -> Result<#parent_name<#next_state_name, T>, ()> {
                                self.observer.on_transition(stringify!(#state_name), stringify!(#next_state_name), Some(data))?;
                                Ok(#parent_name::<#next_state_name, T>::new(#next_state_name::new(data), self.data, self.observer))
                            }
                        }
                    },
                    None => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            fn #event(self, data: #a) -> Result<#parent_name<#next_state_name, T>, ()> {
                                self.observer.on_transition(stringify!(#state_name), stringify!(#next_state_name), Some(data))?;
                                Ok(#parent_name::<#next_state_name, T>::new(#next_state_name::new(data), self.observer))
                            }
                        }
                    }
                },
                None => match shared_data_type {
                    Some(_) => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            fn #event(self) -> Result<#parent_name<#next_state_name, T>, ()> {
                                self.observer.on_transition(stringify!(#state_name), stringify!(#next_state_name), Option::<()>::None)?;
                                Ok(#parent_name::<#next_state_name, T>::new(#next_state_name::new(), self.data, self.observer))
                            }
                        }
                    },
                    None => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            fn #event(self) -> Result<#parent_name<#next_state_name, T>, ()> {
                                self.observer.on_transition(stringify!(#state_name), stringify!(#next_state_name), Option::<()>::None)?;
                                Ok(#parent_name::<#next_state_name, T>::new(#next_state_name::new(), self.observer))
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
                    impl<T: Observer> #parent_name<#state_name, T> {
                        fn new(state: #state_name, data: #sdt, observer: T) -> Self {
                            Self {
                                state,
                                data,
                                observer
                            }
                        }

                        fn data(&self) -> &#sdt {
                            &self.data
                        }
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                fn init(data: #sdt, state_data: #dt, observer: T) -> Result<Self, ()> {
                                    observer.on_init(stringify!(#state_name), Some(data), Some(state_data))?;
                                    Ok(Self::new(#state_name::new(state_data), data, observer))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                fn init(data: #sdt, observer: T) -> Result<Self, ()> {
                                    observer.on_init(stringify!(#state_name), Some(data), Option::<()>::None)?;
                                    Ok(Self::new(#state_name::new(), data, observer))
                                }
                            }
                        }
                    }
                }
            },
            None => {
                let constructor = quote! {
                    impl<T: Observer> #parent_name<#state_name, T> {
                        fn new(state: #state_name, observer: T) -> Self {
                            Self {
                                state,
                                observer
                            }
                        }
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                fn init(state_data: #dt, observer: T) -> Result<Self, ()> {
                                    observer.on_init(stringify!(#state_name), Option::<()>::None, Some(state_data))?;
                                    Ok(Self::new(#state_name::new(state_data), observer))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                fn init(observer: T) -> Result<Self, ()> {
                                    observer.on_init(stringify!(#state_name), Option::<()>::None, Option::<()>::None)?;
                                    Ok(Self::new(#state_name::new(), observer))
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
            struct #parent_name<T, U: Observer> {
                state: T,
                data: #sdt,
                observer: U
            }
        },
        None => quote! {
            struct #parent_name<T, U: Observer> {
                state: T,
                observer: U
            }
        }
    };

    let restore_fns = m.states.iter().map(|x| {
        let state_name = &x.name;
        let expected_state_dt = state_data_types.get(state_name);

        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_lowercase());

        match shared_data_type {
            Some(shared_dt) => {
                match expected_state_dt {
                    Some(state_dt) => quote! {
                        fn #fn_name<T: Observer>(shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<State<T>, ()> {
                            let shared_d_enc_some = shared_d_enc.ok_or(())?;
                            let shared_d: #shared_dt = match shared_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(())?,
                                _ => return Err(())
                            };

                            let state_d_enc_some = state_d_enc.ok_or(())?;
                            let state_d: #state_dt = match state_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(())?,
                                _ => return Err(())
                            };

                            Ok(State::#state_name(#parent_name::<#state_name, T>::new(#state_name::new(state_d), shared_d, observer)))
                        }
                    },
                    None => quote! {
                        fn #fn_name<T: Observer>(shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<State<T>, ()> {
                            let shared_d_enc_some = shared_d_enc.ok_or(())?;
                            let shared_d: #shared_dt = match shared_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(())?,
                                _ => return Err(())
                            };

                            if state_d_enc.is_some() {
                                return Err(())
                            };

                            Ok(State::#state_name(#parent_name::<#state_name, T>::new(#state_name::new(), shared_d, observer)))
                        }
                    }
                }
            },
            None => {
                match expected_state_dt {
                    Some(state_dt) => quote! {
                        fn #fn_name<T: Observer>(shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<State<T>, ()> {
                            if shared_d_enc.is_some() {
                                return Err(())
                            };

                            let state_d_enc_some = state_d_enc.ok_or(())?;
                            let state_d: #state_dt = match state_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(())?,
                                _ => return Err(())
                            };

                            Ok(State::#state_name(#parent_name::<#state_name, T>::new(#state_name::new(state_d), observer)))
                        }
                    },
                    None => quote! {
                        fn #fn_name<T: Observer>(shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<State<T>, ()> {
                            if shared_d_enc.is_some() {
                                return Err(())
                            };

                            if state_d_enc.is_some() {
                                return Err(())
                            };

                            Ok(State::#state_name(#parent_name::<#state_name, T>::new(#state_name::new(), observer)))
                        }
                    }
                }
            }
        }
    });

    let restore_arms = m.states.iter().map(|x| {
        let state_name = &x.name;
        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_lowercase());
        quote!(stringify!(#state_name) => #fn_name(data, state_data, observer))
    });

    let out = quote! {
        trait Observer {
            fn on_init<T: Serialize, U: Serialize>(&self, to: &str, data: Option<T>, state_data: Option<U>) -> Result<(), ()> {
                println!("initializing to {}", to);

                Ok(())
            }
            
            fn on_transition<T: Serialize>(&self, from: &str, to: &str, data: Option<T>) -> Result<(), ()> {
                println!("transitioning from {} to {}", from, to);

                Ok(())
            }
        }

        #parent_struct
        #(#state_structs)*
        #(#parent_state_impls)*
        #transitions_block

        enum State<T: Observer> {
            #(#state_names(#parent_name<#state_names, T>)),*
        }

        enum Encoded {
            Json(String)
        }

        #(#restore_fns)*

        fn restore<T: Observer>(state_str: &str, data: Option<Encoded>, state_data: Option<Encoded>, observer: T) -> Result<State<T>, ()> {
            match state_str {
                #(#restore_arms,)*
                _ => Err(())
            }
        }
    };

    out.into()
}