// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::fmt;

#[cfg(test)]
mod tests;

pub type Result<T> = core::result::Result<(), Error<T>>;

#[cfg(feature = "state-tracing")]
#[doc(hidden)]
pub use tracing::debug as _debug;

#[cfg(not(feature = "state-tracing"))]
#[doc(hidden)]
pub use crate::__tracing_noop__ as _debug;

#[macro_export]
#[doc(hidden)]
macro_rules! __state_transition__ {
    ($state:ident, $valid:pat => $target:expr) => {
        $crate::state::transition!(@build [], _, $state, [$valid => $target])
    };
    (@build [$($targets:expr),*], $event:ident, $state:ident, [$valid:pat => $target:expr] $($remaining:tt)*) => {{
        // if the transition is valid, then perform it
        if matches!($state, $valid) {
            let __event__ = stringify!($event);
            if __event__.is_empty() || __event__ == "_" {
                $crate::state::_debug!(prev = ?$state, next = ?$target, location = %core::panic::Location::caller());
            } else {
                $crate::state::_debug!(event = %__event__, prev = ?$state, next = ?$target, location = %core::panic::Location::caller());
            }

            *$state = $target;
            Ok(())
        } else {
            $crate::state::transition!(
                @build [$($targets,)* $target],
                $event,
                $state,
                $($remaining)*
            )
        }
    }};
    (@build [$($targets:expr),*], $event:ident, $state:ident $(,)?) => {{
        let targets = [$($targets),*];

        // if we only have a single target and the current state matches it, then return a no-op
        if targets.len() == 1 && targets[0].eq($state) {
            let current = targets[0].clone();
            Err($crate::state::Error::NoOp { current })
        } else {
            // if we didn't get a valid match then error out
            Err($crate::state::Error::InvalidTransition {
                current: $state.clone(),
                event: stringify!($event),
            })
        }
    }};
}

pub use crate::__state_transition__ as transition;

#[macro_export]
#[doc(hidden)]
macro_rules! __state_event__ {
    (
        $(#[doc = $doc:literal])*
        $event:ident (
            $(
                $($valid:ident)|* => $target:ident
            ),*
            $(,)?
        )
    ) => {
        $(
            #[doc = $doc]
        )*
        #[inline]
        #[track_caller]
        pub fn $event(&mut self) -> $crate::state::Result<Self> {
            $crate::state::transition!(
                @build [],
                $event,
                self,
                $(
                    [$(Self::$valid)|* => Self::$target]
                )*
            )
        }
    };
    ($(
        $(#[doc = $doc:literal])*
        $event:ident (
            $(
                $($valid:ident)|* => $target:ident
            ),*
            $(,)?
        );
    )*) => {
        $(
            $crate::state::event!(
                $(#[doc = $doc])*
                $event($($($valid)|* => $target),*)
            );
        )*

        #[cfg(test)]
        pub fn test_transitions() -> impl ::core::fmt::Debug {
            use $crate::state::Error;
            use ::core::{fmt, result::Result};

            let mut all_states = [
                // collect all of the states we've observed
                $($(
                    $(
                        (stringify!($valid), Self::$valid),
                    )*
                    (stringify!($target), Self::$target),
                )*)*
            ];

            all_states.sort_unstable_by_key(|v| v.0);
            let (sorted, _) = $crate::slice::partition_dedup(&mut all_states);

            const EVENT_LEN: usize = {
                let mut len = 0;
                $({
                    let _ = stringify!($event);
                    len += 1;
                })*
                len
            };

            let apply = |state: &Self| {
                [$({
                    let mut state = state.clone();
                    let result = state.$event().map(|_| state);
                    (stringify!($event), result)
                }),*]
            };

            struct Transitions<const L: usize, T, A> {
                states: [(&'static str, T); L],
                count: usize,
                apply: A,
            }

            impl<const L: usize, T, A> fmt::Debug for Transitions<L, T, A>
            where
                T: fmt::Debug,
                A: Fn(&T) -> [(&'static str, Result<T, Error<T>>); EVENT_LEN],
            {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    let mut m = f.debug_map();

                    for (name, state) in self.states.iter().take(self.count) {
                        let events = (self.apply)(state);
                        m.entry(&format_args!("{name}"), &Entry(events));
                    }

                    m.finish()
                }
            }

            struct Entry<T>([(&'static str, Result<T, Error<T>>); EVENT_LEN]);

            impl<T> fmt::Debug for Entry<T>
            where
                T: fmt::Debug
            {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    let mut m = f.debug_map();

                    for (event, outcome) in self.0.iter() {
                        m.entry(&format_args!("{event}"), outcome);
                    }

                    m.finish()
                }
            }

            let count = sorted.len();
            Transitions {
                states: all_states,
                count,
                apply,
            }
        }

        /// Generates a dot graph of all state transitions
        pub fn dot() -> impl ::core::fmt::Display {
            struct Dot(&'static str);

            impl ::core::fmt::Display for Dot {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    writeln!(f, "digraph {{")?;
                    writeln!(f, "  label = {:?};", self.0)?;

                    let mut all_states = [
                        // collect all of the states we've observed
                        $($(
                            $(
                                stringify!($valid),
                            )*
                            stringify!($target),
                        )*)*
                    ];

                    all_states.sort_unstable();
                    let (all_states, _) = $crate::slice::partition_dedup(&mut all_states);

                    for state in all_states {
                        writeln!(f, "  {state};")?;
                    }

                    $($(
                        $(
                            writeln!(
                                f,
                                "  {} -> {} [label = {:?}];",
                                stringify!($valid),
                                stringify!($target),
                                stringify!($event),
                            )?;
                        )*
                    )*)*

                    writeln!(f, "}}")?;
                    Ok(())
                }
            }

            Dot(::core::any::type_name::<Self>())
        }
    }
}

pub use crate::__state_event__ as event;

#[macro_export]
#[doc(hidden)]
macro_rules! __state_is__ {
    ($(#[doc = $doc:literal])* $function:ident, $($state:ident)|+) => {
        $(
            #[doc = $doc]
        )*
        #[inline]
        pub fn $function(&self) -> bool {
            matches!(self, $(Self::$state)|*)
        }
    };
}

pub use crate::__state_is__ as is;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error<T> {
    NoOp { current: T },
    InvalidTransition { current: T, event: &'static str },
}

impl<T: fmt::Debug> fmt::Display for Error<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOp { current } => {
                write!(f, "state is already set to {current:?}")
            }
            Self::InvalidTransition { current, event } => {
                write!(f, "invalid event {event:?} for state {current:?}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl<T: fmt::Debug> std::error::Error for Error<T> {}
