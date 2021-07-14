use std::collections::HashMap;
use proc_macro::TokenStream;
use syn::{ parse_macro_input, braced, token, Ident, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::{quote, format_ident};
use convert_case::{Case, Casing};

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

    let state_names: Vec<&Ident> = m.states.iter().map(|x| &x.name).collect();

    let parent_name = &m.name;
    let wrapped_type = format_ident!("{}{}", "Wrapped", parent_name);
    let shared_data_type = &m.shared_data_type;
    
    let state_structs = m.states.iter().map(|x| {
        let state_name = &x.name;
        let data_type = &x.associated_data_type;

        match data_type {
            Some(dt) => quote! {
                pub struct #state_name {
                    data: #dt
                }

                impl #state_name {
                    fn new(data: #dt) -> Self {
                        Self {
                            data
                        }
                    }

                    pub fn data(&self) -> &#dt {
                        &self.data
                    }
                }
            },
            None => quote! {
                pub struct #state_name {}

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
        let exit_fn_name = format_ident!("{}_{}", "on_exit", state_name.to_string().to_case(Case::Snake));

        let transitions = x.transitions.iter().map(|y| {
            let event = &y.event;
            let next_state_name = &y.next_state;
            let arg = state_data_types.get(next_state_name);
            let enter_fn_name = format_ident!("{}_{}", "on_enter", next_state_name.to_string().to_case(Case::Snake));

            let exit_call = match state_data_types.get(state_name) {
                Some(_) => quote! {
                    self.observer.#exit_fn_name(&self.id, State::#next_state_name, self.state.data()).map_err(|e| TransitionError::ObserverError(e))?;
                },
                None => quote! {
                    self.observer.#exit_fn_name(&self.id, State::#next_state_name).map_err(|e| TransitionError::ObserverError(e))?;
                }
            };

            match arg {
                Some(a) => match shared_data_type {
                    Some(_) => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            pub fn #event(mut self, data: #a) -> Result<#parent_name<#next_state_name, T>, TransitionError<T::Error>> {
                                self.observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Some(&data)).map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                self.observer.#enter_fn_name(&self.id, Some(State::#state_name), &data).map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name, T>::new(self.id, #next_state_name::new(data), self.data, self.observer))
                            }
                        }
                    },
                    None => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            pub fn #event(mut self, data: #a) -> Result<#parent_name<#next_state_name, T>, TransitionError<T::Error>> {
                                self.observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Some(&data)).map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                self.observer.#enter_fn_name(&self.id, Some(State::#state_name), &data).map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name, T>::new(self.id, #next_state_name::new(data), self.observer))
                            }
                        }
                    }
                },
                None => match shared_data_type {
                    Some(_) => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            pub fn #event(mut self) -> Result<#parent_name<#next_state_name, T>, TransitionError<T::Error>> {
                                self.observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Option::<()>::None).map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                self.observer.#enter_fn_name(&self.id, Some(State::#state_name)).map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name, T>::new(self.id, #next_state_name::new(), self.data, self.observer))
                            }
                        }
                    },
                    None => quote! {
                        impl<T: Observer> #parent_name<#state_name, T> {
                            pub fn #event(mut self) -> Result<#parent_name<#next_state_name, T>, TransitionError<T::Error>> {
                                self.observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Option::<()>::None).map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                self.observer.#enter_fn_name(&self.id, Some(State::#state_name)).map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name, T>::new(self.id, #next_state_name::new(), self.observer))
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
        let enter_fn_name = format_ident!("{}_{}", "on_enter", state_name.to_string().to_case(Case::Snake));

        let id_fn = quote! {
            pub fn id(&self) -> &str {
                &self.id
            }
        };

        match shared_data_type {
            Some(sdt) => {
                let constructor = quote! {
                    impl<T: Observer> #parent_name<#state_name, T> {
                        fn new(id: String, state: #state_name, data: #sdt, observer: T) -> Self {
                            Self {
                                id,
                                state,
                                data,
                                observer
                            }
                        }

                        #id_fn

                        pub fn data(&self) -> &#sdt {
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
                                pub fn init(id: Option<String>, data: #sdt, state_data: #dt, mut observer: T) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Some(&data), Some(&state_data)).map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None, &state_data).map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(state_data), data, observer))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                pub fn init(id: Option<String>, data: #sdt, mut observer: T) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Some(&data), Option::<()>::None).map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None).map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(), data, observer))
                                }
                            }
                        }
                    }
                }
            },
            None => {
                let constructor = quote! {
                    impl<T: Observer> #parent_name<#state_name, T> {
                        fn new(id: String, state: #state_name, observer: T) -> Self {
                            Self {
                                id,
                                state,
                                observer
                            }
                        }

                        #id_fn
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                pub fn init(id: Option<String>, state_data: #dt, mut observer: T) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Option::<()>::None, Some(&state_data)).map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None, &state_data).map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(state_data), observer))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl<T: Observer> #parent_name<#state_name, T> {
                                pub fn init(id: Option<String>, mut observer: T) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Option::<()>::None, Option::<()>::None).map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None).map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(), observer))
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
            pub struct #parent_name<T, U: Observer> {
                id: String,
                pub state: T,
                data: #sdt,
                observer: U
            }
        },
        None => quote! {
            pub struct #parent_name<T, U: Observer> {
                id: String,
                pub state: T,
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
                        fn #fn_name<T: Observer>(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<#wrapped_type<T>, RestoreError> {
                            let shared_d_enc_some = shared_d_enc.ok_or(RestoreError::EmptyData)?;
                            let shared_d: #shared_dt = match shared_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            let state_d_enc_some = state_d_enc.ok_or(RestoreError::EmptyData)?;
                            let state_d: #state_dt = match state_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name, T>::new(id, #state_name::new(state_d), shared_d, observer)))
                        }
                    },
                    None => quote! {
                        fn #fn_name<T: Observer>(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<#wrapped_type<T>, RestoreError> {
                            let shared_d_enc_some = shared_d_enc.ok_or(RestoreError::EmptyData)?;
                            let shared_d: #shared_dt = match shared_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            if state_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name, T>::new(id, #state_name::new(), shared_d, observer)))
                        }
                    }
                }
            },
            None => {
                match expected_state_dt {
                    Some(state_dt) => quote! {
                        fn #fn_name<T: Observer>(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<#wrapped_type<T>, RestoreError> {
                            if shared_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            let state_d_enc_some = state_d_enc.ok_or(RestoreError::EmptyData)?;
                            let state_d: #state_dt = match state_d_enc_some {
                                Encoded::Json(data) => serde_json::from_str(&data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name, T>::new(id, #state_name::new(state_d), observer)))
                        }
                    },
                    None => quote! {
                        fn #fn_name<T: Observer>(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>, observer: T) -> Result<#wrapped_type<T>, RestoreError> {
                            if shared_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            if state_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name, T>::new(id, #state_name::new(), observer)))
                        }
                    }
                }
            }
        }
    });

    let restore_arms = m.states.iter().map(|x| {
        let state_name = &x.name;
        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_lowercase());
        quote!(stringify!(#state_name) => #fn_name(id, data, state_data, observer))
    });

    let listeners = m.states.iter().map(|x| {
        let state_name = &x.name;
        let enter_fn_name = format_ident!("{}_{}", "on_enter", state_name.to_string().to_case(Case::Snake));
        let exit_fn_name = format_ident!("{}_{}", "on_exit", state_name.to_string().to_case(Case::Snake));

        let maybe_data_type = state_data_types.get(state_name);
        match maybe_data_type {
            Some(data_type) => quote! {
                fn #enter_fn_name(&self, id: &str, from: Option<State>, data: &#data_type) -> Result<(), Self::Error> {
                    Ok(())
                }
                fn #exit_fn_name(&self, id: &str, to: State, data: &#data_type) -> Result<(), Self::Error> {
                    Ok(())
                }
            },
            None => quote! {
                fn #enter_fn_name(&self, id: &str, from: Option<State>) -> Result<(), Self::Error> {
                    Ok(())
                }
                fn #exit_fn_name(&self, id: &str, to: State) -> Result<(), Self::Error> {
                    Ok(())
                }
            }
        }

    });

    let out = quote! {
        #[derive(Debug)]
        pub enum InitError<T> {
            EmptyId,
            ObserverError(T)
        }

        #[derive(Debug)]
        pub enum TransitionError<T> {
            ObserverError(T)
        }

        #[derive(Debug)]
        pub enum RestoreError {
            EmptyData,
            UnexpectedData,
            InvalidData,
            InvalidState
        }
        
        pub enum State {
            #(#state_names),*
        }

        impl State {
            pub fn to_string(&self) -> String {
                match self {
                    #(State::#state_names => String::from(stringify!(#state_names))),*
                }
            }
        }
        
        pub trait Observer {
            type Error;

            fn on_init<T: Serialize, U: Serialize>(&mut self, id: Option<String>, to: State, data: Option<T>, state_data: Option<U>) -> Result<Option<String>, Self::Error> {
                Ok(id)
            }
            
            fn on_transition<T: Serialize>(&mut self, id: &str, from: State, to: State, data: Option<T>) -> Result<(), Self::Error> {
                Ok(())
            }

            #(#listeners)*
        }

        #parent_struct
        #(#state_structs)*
        #(#parent_state_impls)*
        #transitions_block

        pub enum #wrapped_type<T: Observer> {
            #(#state_names(#parent_name<#state_names, T>)),*
        }

        pub enum Encoded<'a> {
            Json(&'a str)
        }

        #(#restore_fns)*

        pub fn restore<T: Observer>(id: String, state_str: &str, data: Option<Encoded>, state_data: Option<Encoded>, observer: T) -> Result<#wrapped_type<T>, RestoreError> {
            match state_str {
                #(#restore_arms,)*
                _ => Err(RestoreError::InvalidState)
            }
        }
    };

    out.into()
}