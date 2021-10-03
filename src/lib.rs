use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{braced, parse_macro_input, token, Ident, Result, Token};

struct Machine {
    name: Ident,
    #[allow(dead_code)]
    shared_data_type: Option<Ident>,
    #[allow(dead_code)]
    brace_token: token::Brace,
    states: Punctuated<StateDefinition, Token![,]>,
}

struct StateDefinition {
    init: bool,
    name: Ident,
    #[allow(dead_code)]
    associated_data_type: Option<Ident>,
    #[allow(dead_code)]
    brace_token: token::Brace,
    transitions: Punctuated<StateTransition, Token![,]>,
}

struct StateTransition {
    event: Ident,
    #[allow(dead_code)]
    separator: Token![=>],
    next_state: Ident,
}

impl Parse for Machine {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;

        let mut shared_data_type: Option<Ident> = None;
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

        let mut associated_data_type: Option<Ident> = None;
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
            next_state: input.parse()?,
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
        state_data_types.insert(&state.name, format_ident!("{}{}", &state.name, "Data"));
    }

    let state_names: Vec<&Ident> = m.states.iter().map(|x| &x.name).collect();
    let state_datas: Vec<Ident> = m
        .states
        .iter()
        .map(|x| format_ident!("{}{}", &x.name, "Data"))
        .collect();

    let parent_name = &m.name;
    let wrapped_type = format_ident!("{}{}", "Wrapped", parent_name);

    let state_structs = m.states.iter().map(|x| {
        let state_name = &x.name;
        let data_type = state_data_types.get(state_name).unwrap();

        quote! {
            pub struct #state_name<S, T: Observer<S>> {
                data: T::#data_type,
                // phantom: PhantomData<S>
            }

            impl<S, T: Observer<S>> #state_name<S, T> {
                fn new(data: T::#data_type) -> Self {
                    Self {
                        data,
                        // phantom: PhantomData
                    }
                }

                pub fn data(&self) -> &T::#data_type {
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
            let arg = state_data_types.get(next_state_name).unwrap();
            let enter_fn_name = format_ident!("{}_{}", "on_enter", next_state_name.to_string().to_case(Case::Snake));

            let exit_call = quote! {
                self.observer.#exit_fn_name(ctx, &self.id, State::#next_state_name(&data), &self.data, &self.state.data).map_err(|e| TransitionError::ObserverError(e))?;
            };

            let enter_from_type = if init_states.contains(next_state_name) {
                quote!(Some(State::#state_name(self.state.data())))
            } else {
                quote!(State::#state_name(self.state.data()))
            };

            quote! {
                impl<S, T: Observer<S>> #parent_name<#state_name<S, T>, S, T> {
                    pub fn #event(mut self, ctx: &mut S, data: T::#arg) -> Result<#parent_name<#next_state_name<S, T>, S, T>, TransitionError<T::Error>> {
                        self.observer.on_transition(ctx, &self.id, State::#state_name(self.state.data()), State::#next_state_name(&data), &self.data).map_err(|e| TransitionError::ObserverError(e))?;
                        #exit_call
                        self.observer.#enter_fn_name(ctx, &self.id, #enter_from_type, &self.data, &data).map_err(|e| TransitionError::ObserverError(e))?;
                        Ok(#parent_name::<#next_state_name<S, T>, S, T>::new(self.observer, self.id, #next_state_name::<S, T>::new(data), self.data))
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
        let state_data_type = state_data_types.get(state_name).unwrap();

        let common_methods = quote! {
            pub fn id(&self) -> &str {
                &self.id
            }
        };

        let constructor = quote! {
            impl<S, T: Observer<S>> #parent_name<#state_name<S, T>, S, T> {
                fn new(observer: T, id: String, state: #state_name<S, T>, data: T::Data) -> Self {
                    Self {
                        observer,
                        id,
                        state,
                        data,
                        // phantom: PhantomData
                    }
                }

                pub fn data(&self) -> &T::Data {
                    &self.data
                }

                #common_methods
            }
        };

        match x.init {
            false => constructor,
            true => quote! {
                #constructor

                impl<S, T: Observer<S>> #parent_name<#state_name<S, T>, S, T> {
                    pub fn init(ctx: &mut S, mut observer: T, id: Option<String>, data: T::Data, state_data: T::#state_data_type) -> Result<Self, InitError<T::Error>> {
                        let id = observer.on_init(ctx, id, State::#state_name(&state_data), &data).map_err(|e| InitError::ObserverError(e))?.ok_or(InitError::EmptyId)?;
                        observer.#enter_fn_name(ctx, &id, None, &data, &state_data).map_err(|e| InitError::ObserverError(e))?;
                        Ok(Self::new(observer, id, #state_name::<S, T>::new(state_data), data))
                    }
                }
            }
        }
    });

    let parent_struct = quote! {
        pub struct #parent_name<S, T, U: Observer<T>> {
            observer: U,
            id: String,
            pub state: S,
            data: U::Data,
            // phantom: PhantomData<T>
        }
    };

    let restore_fns = m.states.iter().map(|x| {
        let state_name = &x.name;
        let state_dt = state_data_types.get(state_name).unwrap();

        let fn_name = format_ident!("{}_{}", "restore", state_name.to_string().to_case(Case::Snake));
        quote! {
            fn #fn_name<S, T: Observer<S>>(mut observer: T, id: String, shared_d_enc: Encoded, state_d_enc: Encoded) -> Result<#wrapped_type<S, T>, RestoreError> {
                let shared_d: T::Data = match shared_d_enc {
                    Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                };

                let state_d: T::#state_dt = match state_d_enc {
                    Encoded::Json(data) => serde_json::from_value(data).ok().ok_or(RestoreError::InvalidData)?
                };

                Ok(#wrapped_type::#state_name(#parent_name::<#state_name<S, T>, S, T>::new(observer, id, #state_name::<S, T>::new(state_d), shared_d)))
            }
        }
    });

    let restore_arms = m.states.iter().map(|x| {
        let state_name = &x.name;
        let fn_name = format_ident!(
            "{}_{}",
            "restore",
            state_name.to_string().to_case(Case::Snake)
        );
        quote!(stringify!(#state_name) => #fn_name(observer, id, data, state_data))
    });

    let listeners = m.states.iter().map(|x| {
        let state_name = &x.name;
        let enter_fn_name = format_ident!("{}_{}", "on_enter", state_name.to_string().to_case(Case::Snake));
        let exit_fn_name = format_ident!("{}_{}", "on_exit", state_name.to_string().to_case(Case::Snake));

        let data_type = state_data_types.get(state_name).unwrap();

        let from_type = if x.init {
            quote!(Option<State<'a, S, Self>>)
        } else {
            quote!(State<'a, S, Self>)
        };

        quote! {
            fn #enter_fn_name<'a>(&mut self, ctx: &mut S, id: &str, from: #from_type, data: &Self::Data, state_data: &Self::#data_type) -> Result<(), Self::Error> where Self: Sized {
                Ok(())
            }
            fn #exit_fn_name<'a>(&mut self, ctx: &mut S, id: &str, to: State<'a, S, Self>, data: &Self::Data, state_data: &Self::#data_type) -> Result<(), Self::Error> where Self: Sized {
                Ok(())
            }
        }
    });

    let state_enum_types = m.states.iter().map(|s| {
        let state_name = &s.name;
        let data = state_data_types.get(state_name).unwrap();

        quote! {
            #state_name(&'a T::#data)
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

        pub enum State<'a, S, T: Observer<S>> {
            #(#state_enum_types),*
        }

        impl<'a, S, T: Observer<S>> State<'a, S, T> {
            pub fn to_string(&self) -> String {
                match self {
                    #(State::#state_names(_) => String::from(stringify!(#state_names))),*
                }
            }
        }

        pub trait Observer<S> {
            type Data: Serialize + DeserializeOwned;
            #(type #state_datas: Serialize + DeserializeOwned;)*
            type Error;

            fn on_init<'a>(&mut self, ctx: &mut S, id: Option<String>, to: State<'a, S, Self>, data: &Self::Data) -> Result<Option<String>, Self::Error> where Self: Sized {
                Ok(id)
            }

            fn on_transition<'a>(&mut self, ctx: &mut S, id: &str, from: State<'a, S, Self>, to: State<'a, S, Self>, data: &Self::Data) -> Result<(), Self::Error> where Self: Sized {
                Ok(())
            }

            #(#listeners)*
        }

        pub trait Retriever<T> {
            type Error;

            fn on_retrieve(&mut self, ctx: &mut T, id: &str) -> Result<(String, Encoded, Encoded), Self::Error>;
        }

        #parent_struct
        #(#state_structs)*
        #(#parent_state_impls)*
        #transitions_block

        pub enum #wrapped_type<S, T: Observer<S>> {
            #(#state_names(#parent_name<#state_names<S, T>, S, T>)),*
        }

        pub enum Encoded {
            Json(serde_json::Value)
        }

        #(#restore_fns)*

        pub fn restore<S, T: Observer<S>>(mut observer: T, id: String, state_string: String, data: Encoded, state_data: Encoded) -> Result<#wrapped_type<S, T>, RestoreError> {
            let state_str: &str = &state_string;
            match state_str {
                #(#restore_arms,)*
                _ => Err(RestoreError::InvalidState)
            }
        }

        pub fn retrieve<S, T: Retriever<S> + Observer<S>>(ctx: &mut S, mut retriever: T, id: String) -> Result<#wrapped_type<S, T>, RetrieveError<<T as Retriever<S>>::Error>> {
            let id_str: &str = &id;
            let (state_string, data, state_data) = retriever.on_retrieve(ctx, id_str).map_err(|e| RetrieveError::RetrieverError(e))?;
            restore(retriever, id, state_string, data, state_data).map_err(|e| RetrieveError::RestoreError(e))
        }
    };

    out.into()
}
