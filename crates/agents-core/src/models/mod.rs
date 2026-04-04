//! Provider-agnostic model abstractions and routing helpers.

pub mod interface;
pub mod multi_provider;

pub use interface::{Model, ModelProvider, ModelRequest, ModelResponse};
pub use multi_provider::{
    MultiProvider, MultiProviderMap, MultiProviderOpenAIPrefixMode, MultiProviderUnknownPrefixMode,
};
