use std::collections::{HashMap, HashSet};
use proc_macro::TokenStream;
use syn::{ parse_macro_input, braced, Ident, Type, token, Result, Token };
use syn::parse::{ Parse, ParseStream };
use syn::punctuated::Punctuated;
use quote::{quote, format_ident};
use convert_case::{Case, Casing};

struct Machine {
    name: Ident,
    shared_data_type: Option<Type>,
    #[allow(dead_code)]
    brace_token: token::Brace,
    states: Punctuated<StateDefinition, Token![,]>
}

struct StateDefinition {
    init: bool,
    name: Ident,
    associated_data_type: Option<Type>,
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
        
        let mut shared_data_type: Option<Type> = None;
        let colon: Result<Token![:]> = input.parse();
        if colon.is_ok() {
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

        let mut associated_data_type: Option<Type> = None;
        let colon: Result<Token![:]> = input.parse();
        if colon.is_ok() {
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
    let mut init_states = HashSet::new();
    let mut state_data_types = HashMap::new();
    
    for state in m.states.iter() {
        if state.init {
            init_states.insert(&state.name);
        };
        match &state.associated_data_type {
            Some(dt) => state_data_types.insert(&state.name, dt.to_owned()),
            None => state_data_types.insert(&state.name, Type::Verbatim(quote!(())))
        };
    }

    let state_names: Vec<&Ident> = m.states.iter().map(|x| &x.name).collect();
    let parent_name = &m.name;
    let wrapped_type = format_ident!("{}{}", "Wrapped", parent_name);
    let shared_data_type = m.shared_data_type.or(Some(Type::Verbatim(quote!(())))).unwrap();
    
    let state_structs = m.states.iter().map(|x| {
        let state_name = &x.name;
        let state_data_type = state_data_types.get(state_name).unwrap();

        quote! {
            pub struct #state_name {
                data: #state_data_type
            }

            impl #state_name {
                fn new(data: #state_data_type) -> Self {
                    Self {
                        data
                    }
                }

                pub fn data(&self) -> &#state_data_type {
                    &self.data
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
            let next_state_data_type = state_data_types.get(next_state_name).unwrap();
            let enter_fn_name = format_ident!("{}_{}", "on_enter", next_state_name.to_string().to_case(Case::Snake));

            let enter_from_value = if init_states.contains(next_state_name) {
                quote!(Some(State::#state_name(self.state.data())))
            } else {
                quote!(State::#state_name(self.state.data()))
            };

            quote! {
                impl<S: Send, T: Observer<S> + Send> #parent_name<#state_name, S, T> {
                    pub async fn #event(mut self, ctx: &mut S, next_state_data: #next_state_data_type) -> Result<#parent_name<#next_state_name, S, T>, TransitionError<T::Error>> {
                        self.observer.on_transition(ctx, State::#state_name(self.state.data()), State::#next_state_name(&next_state_data), &self.id, &self.data).await.map_err(|e| TransitionError::ObserverError(e))?;
                        self.observer.#exit_fn_name(ctx, State::#next_state_name(&next_state_data), &self.id, &self.data, &self.state.data).await.map_err(|e| TransitionError::ObserverError(e))?;
                        self.observer.#enter_fn_name(ctx, #enter_from_value, &self.id, &self.data, &next_state_data).await.map_err(|e| TransitionError::ObserverError(e))?;
                        Ok(#parent_name::<#next_state_name, S, T>::new(self.observer, self.id, #next_state_name::new(next_state_data), self.data))
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
        let state_data_type = state_data_types.get(state_name).unwrap();
        let enter_fn_name = format_ident!("{}_{}", "on_enter", state_name.to_string().to_case(Case::Snake));

        let constructor = quote! {
            impl<S: Send, T: Observer<S> + Send> #parent_name<#state_name, S, T> {
                fn new(observer: T, id: T::ID, state: #state_name, data: #shared_data_type) -> Self {
                    Self {
                        observer,
                        id,
                        state,
                        data,
                        phantom: PhantomData
                    }
                }

                pub fn id(&self) -> &T::ID {
                    &self.id
                }

                pub fn data(&self) -> &#shared_data_type {
                    &self.data
                }
            }
        };

        match x.init {
            false => constructor,
            true => quote! {
                #constructor

                impl<S: Send, T: Observer<S> + Send> #parent_name<#state_name, S, T> {
                    pub async fn init(ctx: &mut S, mut observer: T, id: Option<T::ID>, shared_data: #shared_data_type, state_data: #state_data_type) -> Result<Self, InitError<T::Error>> {
                        let id = observer.on_init(ctx, State::#state_name(&state_data), id, &shared_data).await.map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                        observer.#enter_fn_name(ctx, None, &id, &shared_data, &state_data).await.map_err(|e| InitError::ObserverError(e))?;
                        Ok(Self::new(observer, id, #state_name::new(state_data), shared_data))
                    }
                }
            }
        }
    });

    let parent_struct = quote! {
        pub struct #parent_name<S, T: Send, U: Observer<T> + Send> {
            observer: U,
            id: U::ID,
            pub state: S,
            data: #shared_data_type,
            phantom: PhantomData<T>
        }
    };

    let restore_fns = m.states.iter().map(|x| {
        let state_name = &x.name;
        let state_data_type = state_data_types.get(state_name).unwrap();
        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_case(Case::Snake));

        quote! {
            async fn #fn_name<S: Send, T: Observer<S> + Send>(mut observer: T, id: T::ID, shared_d_enc: Encoded, state_d_enc: Encoded) -> Result<#wrapped_type<S, T>, RestoreError> {
                let shared_d: #shared_data_type = match shared_d_enc {
                    Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                };

                let state_d: #state_data_type = match state_d_enc {
                    Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                };

                Ok(#wrapped_type::#state_name(#parent_name::<#state_name, S, T>::new(observer, id, #state_name::new(state_d), shared_d)))
            }
        }
    });

    let restore_arms = m.states.iter().map(|x| {
        let state_name = &x.name;
        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_case(Case::Snake));
        
        quote!(stringify!(#state_name) => #fn_name(observer, id, shared_data, state_data).await)
    });

    let listeners = m.states.iter().map(|x| {
        let state_name = &x.name;
        let state_data_type = state_data_types.get(state_name).unwrap();
        let enter_fn_name = format_ident!("{}_{}", "on_enter", state_name.to_string().to_case(Case::Snake));
        let exit_fn_name = format_ident!("{}_{}", "on_exit", state_name.to_string().to_case(Case::Snake));

        let from_type = if x.init {
            quote!(Option<State<'a>>)
        } else {
            quote!(State<'a>)
        };
        
        quote! {
            async fn #enter_fn_name<'a>(&mut self, ctx: &mut S, from: #from_type, id: &Self::ID, data: &#shared_data_type, state_data: &#state_data_type) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn #exit_fn_name<'a>(&mut self, ctx: &mut S, to: State<'a>, id: &Self::ID, data: &#shared_data_type, state_data: &#state_data_type) -> Result<(), Self::Error> {
                Ok(())
            }
        }
    });

    let state_enum_types = state_names.iter().map(|&state_name| {
        let state_data_type = state_data_types.get(state_name).unwrap();

        quote! {
            #state_name(&'a #state_data_type)
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
        
        pub enum State<'a> {
            #(#state_enum_types),*
        }

        impl<'a> State<'a> {
            pub fn to_string(&self) -> String {
                match self {
                    #(State::#state_names(_) => String::from(stringify!(#state_names))),*
                }
            }

            pub fn data_as_json(&self) -> Result<serde_json::Value, serde_json::Error> {
                match self {
                    #(State::#state_names(data) => serde_json::to_value(data)),*
                }
            }
        }
        
        #[async_trait]
        pub trait Observer<S: Send> {
            type ID: Send + Sync;
            type Error;

            async fn on_init<'a>(&mut self, ctx: &mut S, to: State<'a>, id: Option<Self::ID>, data: &#shared_data_type) -> Result<Option<Self::ID>, Self::Error> {
                Ok(id)
            }
            
            async fn on_transition<'a>(&mut self, ctx: &mut S, from: State<'a>, to: State<'a>, id: &Self::ID, data: &#shared_data_type) -> Result<(), Self::Error> {
                Ok(())
            }

            #(#listeners)*
        }

        #[async_trait]
        pub trait Retriever<T: Send, U: Observer<T> + Send> {
            type Error;

            async fn on_retrieve(&mut self, ctx: &mut T, id: &U::ID) -> Result<(String, Encoded, Encoded), Self::Error>;
        }

        #parent_struct
        #(#state_structs)*
        #(#parent_state_impls)*
        #transitions_block

        pub enum #wrapped_type<S: Send, T: Observer<S> + Send> {
            #(#state_names(#parent_name<#state_names, S, T>)),*
        }

        pub enum Encoded {
            Json(serde_json::Value)
        }

        #(#restore_fns)*

        pub async fn restore<S: Send, T: Observer<S> + Send>(mut observer: T, id: T::ID, state_string: String, shared_data: Encoded, state_data: Encoded) -> Result<#wrapped_type<S, T>, RestoreError> {
            let state_str: &str = &state_string;
            
            match state_str {
                #(#restore_arms,)*
                _ => Err(RestoreError::InvalidState)
            }
        }

        pub async fn retrieve<S: Send, T: Observer<S> + Retriever<S, T> + Send>(ctx: &mut S, mut retriever: T, id: T::ID) -> Result<#wrapped_type<S, T>, RetrieveError<<T as Retriever<S, T>>::Error>> {
            let (state_string, shared_data, state_data) = retriever.on_retrieve(ctx, &id).await.map_err(|e| RetrieveError::RetrieverError(e))?;
            
            restore(retriever, id, state_string, shared_data, state_data).await.map_err(|e| RetrieveError::RestoreError(e))
        }
    };

    out.into()
}