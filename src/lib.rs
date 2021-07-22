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

            let exit_call = match shared_data_type {
                Some(_) => match state_data_types.get(state_name) {
                    Some(_) => quote! {
                        observer.#exit_fn_name(&self.id, State::#next_state_name, &self.data, &self.state.data).await.map_err(|e| TransitionError::ObserverError(e))?;
                    },
                    None => quote! {
                        observer.#exit_fn_name(&self.id, State::#next_state_name, &self.data).await.map_err(|e| TransitionError::ObserverError(e))?;
                    }
                },
                None => match state_data_types.get(state_name) {
                    Some(_) => quote! {
                        observer.#exit_fn_name(&self.id, State::#next_state_name, &self.state.data).await.map_err(|e| TransitionError::ObserverError(e))?;
                    },
                    None => quote! {
                        observer.#exit_fn_name(&self.id, State::#next_state_name).await.map_err(|e| TransitionError::ObserverError(e))?;
                    }
                }
            };

            match arg {
                Some(a) => match shared_data_type {
                    Some(_) => quote! {
                        impl #parent_name<#state_name> {
                            pub async fn #event<T: Observer + Send>(mut self, mut observer: T, data: #a) -> Result<#parent_name<#next_state_name>, TransitionError<T::Error>> {
                                observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Some(&self.data), Some(&data)).await.map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                observer.#enter_fn_name(&self.id, Some(State::#state_name), &self.data, &data).await.map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name>::new(self.id, #next_state_name::new(data), self.data))
                            }
                        }
                    },
                    None => quote! {
                        impl #parent_name<#state_name> {
                            pub async fn #event<T: Observer + Send>(mut self, mut observer: T, data: #a) -> Result<#parent_name<#next_state_name>, TransitionError<T::Error>> {
                                observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Option::<()>::None, Some(&data)).await.map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                observer.#enter_fn_name(&self.id, Some(State::#state_name), &data).await.map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name>::new(self.id, #next_state_name::new(data)))
                            }
                        }
                    }
                },
                None => match shared_data_type {
                    Some(_) => quote! {
                        impl #parent_name<#state_name> {
                            pub async fn #event<T: Observer + Send>(mut self, mut observer: T) -> Result<#parent_name<#next_state_name>, TransitionError<T::Error>> {
                                observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Some(&self.data), Option::<()>::None).await.map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                observer.#enter_fn_name(&self.id, Some(State::#state_name), &self.data).await.map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name>::new(self.id, #next_state_name::new(), self.data))
                            }
                        }
                    },
                    None => quote! {
                        impl #parent_name<#state_name> {
                            pub async fn #event<T: Observer + Send>(mut self, mut observer: T) -> Result<#parent_name<#next_state_name>, TransitionError<T::Error>> {
                                observer.on_transition(&self.id, State::#state_name, State::#next_state_name, Option::<()>::None, Option::<()>::None).await.map_err(|e| TransitionError::ObserverError(e))?;
                                #exit_call
                                observer.#enter_fn_name(&self.id, Some(State::#state_name)).await.map_err(|e| TransitionError::ObserverError(e))?;
                                Ok(#parent_name::<#next_state_name>::new(self.id, #next_state_name::new()))
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

        let common_methods = quote! {
            pub fn id(&self) -> &str {
                &self.id
            }
        };

        match shared_data_type {
            Some(sdt) => {
                let constructor = quote! {
                    impl #parent_name<#state_name> {
                        fn new(id: String, state: #state_name, data: #sdt) -> Self {
                            Self {
                                id,
                                state,
                                data
                            }
                        }

                        pub fn data(&self) -> &#sdt {
                            &self.data
                        }

                        #common_methods
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                pub async fn init<T: Observer + Send>(mut observer: T, id: Option<String>, data: #sdt, state_data: #dt) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Some(&data), Some(&state_data)).await.map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None, &data, &state_data).await.map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(state_data), data))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                pub async fn init<T: Observer + Send>(mut observer: T, id: Option<String>, data: #sdt) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Some(&data), Option::<()>::None).await.map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None, &data).await.map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(), data))
                                }
                            }
                        }
                    }
                }
            },
            None => {
                let constructor = quote! {
                    impl #parent_name<#state_name> {
                        fn new(id: String, state: #state_name) -> Self {
                            Self {
                                id,
                                state
                            }
                        }

                        #common_methods
                    }
                };

                match x.init {
                    false => constructor,
                    true => match &x.associated_data_type {
                        Some(dt) => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                pub async fn init<T: Observer + Send>(mut observer: T, id: Option<String>, state_data: #dt) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Option::<()>::None, Some(&state_data)).await.map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None, &state_data).await.map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new(state_data)))
                                }
                            }
                        },
                        None => quote! {
                            #constructor
    
                            impl #parent_name<#state_name> {
                                pub async fn init<T: Observer + Send>(mut observer: T, id: Option<String>) -> Result<Self, InitError<T::Error>> {
                                    let id = observer.on_init(id, State::#state_name, Option::<()>::None, Option::<()>::None).await.map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                                    observer.#enter_fn_name(&id, None).await.map_err(|e| InitError::ObserverError(e))?;
                                    Ok(Self::new(id, #state_name::new()))
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
            pub struct #parent_name<T> {
                id: String,
                pub state: T,
                data: #sdt
            }
        },
        None => quote! {
            pub struct #parent_name<T> {
                id: String,
                pub state: T
            }
        }
    };

    let restore_fns = m.states.iter().map(|x| {
        let state_name = &x.name;
        let expected_state_dt = state_data_types.get(state_name);

        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_case(Case::Snake));

        match shared_data_type {
            Some(shared_dt) => {
                match expected_state_dt {
                    Some(state_dt) => quote! {
                        async fn #fn_name(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>) -> Result<#wrapped_type, RestoreError> {
                            let shared_d_enc_some = shared_d_enc.ok_or(RestoreError::EmptyData)?;
                            let shared_d: #shared_dt = match shared_d_enc_some {
                                Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            let state_d_enc_some = state_d_enc.ok_or(RestoreError::EmptyData)?;
                            let state_d: #state_dt = match state_d_enc_some {
                                Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name>::new(id, #state_name::new(state_d), shared_d)))
                        }
                    },
                    None => quote! {
                        async fn #fn_name(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>) -> Result<#wrapped_type, RestoreError> {
                            let shared_d_enc_some = shared_d_enc.ok_or(RestoreError::EmptyData)?;
                            let shared_d: #shared_dt = match shared_d_enc_some {
                                Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            if state_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name>::new(id, #state_name::new(), shared_d)))
                        }
                    }
                }
            },
            None => {
                match expected_state_dt {
                    Some(state_dt) => quote! {
                        async fn #fn_name(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>) -> Result<#wrapped_type, RestoreError> {
                            if shared_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            let state_d_enc_some = state_d_enc.ok_or(RestoreError::EmptyData)?;
                            let state_d: #state_dt = match state_d_enc_some {
                                Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name>::new(id, #state_name::new(state_d))))
                        }
                    },
                    None => quote! {
                        async fn #fn_name(id: String, shared_d_enc: Option<Encoded>, state_d_enc: Option<Encoded>) -> Result<#wrapped_type, RestoreError> {
                            if shared_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            if state_d_enc.is_some() {
                                return Err(RestoreError::UnexpectedData)
                            };

                            Ok(#wrapped_type::#state_name(#parent_name::<#state_name>::new(id, #state_name::new())))
                        }
                    }
                }
            }
        }
    });

    let restore_arms = m.states.iter().map(|x| {
        let state_name = &x.name;
        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_case(Case::Snake));
        quote!(stringify!(#state_name) => #fn_name(id, data, state_data).await)
    });

    let listeners = m.states.iter().map(|x| {
        let state_name = &x.name;
        let enter_fn_name = format_ident!("{}_{}", "on_enter", state_name.to_string().to_case(Case::Snake));
        let exit_fn_name = format_ident!("{}_{}", "on_exit", state_name.to_string().to_case(Case::Snake));

        let maybe_data_type = state_data_types.get(state_name);
        match shared_data_type {
            Some(sdt) => match maybe_data_type {
                Some(data_type) => quote! {
                    async fn #enter_fn_name(&mut self, id: &str, from: Option<State>, data: &#sdt, state_data: &#data_type) -> Result<(), Self::Error> {
                        Ok(())
                    }
                    async fn #exit_fn_name(&mut self, id: &str, to: State, data: &#sdt, state_data: &#data_type) -> Result<(), Self::Error> {
                        Ok(())
                    }
                },
                None => quote! {
                    async fn #enter_fn_name(&mut self, id: &str, from: Option<State>, data: &#sdt) -> Result<(), Self::Error> {
                        Ok(())
                    }
                    async fn #exit_fn_name(&mut self, id: &str, to: State, data: &#sdt) -> Result<(), Self::Error> {
                        Ok(())
                    }
                }
            },
            None => match maybe_data_type {
                Some(data_type) => quote! {
                    async fn #enter_fn_name(&mut self, id: &str, from: Option<State>, state_data: &#data_type) -> Result<(), Self::Error> {
                        Ok(())
                    }
                    async fn #exit_fn_name(&mut self, id: &str, to: State, state_data: &#data_type) -> Result<(), Self::Error> {
                        Ok(())
                    }
                },
                None => quote! {
                    async fn #enter_fn_name(&mut self, id: &str, from: Option<State>) -> Result<(), Self::Error> {
                        Ok(())
                    }
                    async fn #exit_fn_name(&mut self, id: &str, to: State) -> Result<(), Self::Error> {
                        Ok(())
                    }
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

        #[derive(Debug)]
        pub enum RetrieveError<T> {
            RestoreError(RestoreError),
            RetrieverError(T)
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
        
        #[async_trait]
        pub trait Observer {
            type Error;

            async fn on_init<T: Serialize + Send, U: Serialize + Send>(&mut self, id: Option<String>, to: State, data: Option<T>, state_data: Option<U>) -> Result<Option<String>, Self::Error> {
                Ok(id)
            }
            
            async fn on_transition<T: Serialize + Send, U: Serialize + Send>(&mut self, id: &str, from: State, to: State, data: Option<T>, state_data: Option<U>) -> Result<(), Self::Error> {
                Ok(())
            }

            #(#listeners)*
        }

        #[async_trait]
        pub trait Retriever {
            type RetrieverError;

            async fn on_retrieve(&mut self, id: &str) -> Result<(String, Option<Encoded>, Option<Encoded>), Self::RetrieverError>;
        }

        #parent_struct
        #(#state_structs)*
        #(#parent_state_impls)*
        #transitions_block

        pub enum #wrapped_type {
            #(#state_names(#parent_name<#state_names>)),*
        }

        pub enum Encoded {
            Json(serde_json::Value)
        }

        #(#restore_fns)*

        pub async fn restore(id: String, state_string: String, data: Option<Encoded>, state_data: Option<Encoded>) -> Result<#wrapped_type, RestoreError> {
            let state_str: &str = &state_string;
            match state_str {
                #(#restore_arms,)*
                _ => Err(RestoreError::InvalidState)
            }
        }

        pub async fn retrieve<T: Retriever + Send>(id: String, mut retriever: T) -> Result<#wrapped_type, RetrieveError<T::RetrieverError>> {
            let id_str: &str = &id;
            let (state_string, maybe_data, maybe_state_data) = retriever.on_retrieve(id_str).await.map_err(|e| RetrieveError::RetrieverError(e))?;
            restore(id, state_string, maybe_data, maybe_state_data).await.map_err(|e| RetrieveError::RestoreError(e))
        }
    };

    out.into()
}