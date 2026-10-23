//! lemonfiber's adapter for SABnzbd: the `download.usenet` contract, spoken in front of
//! one SABnzbd and translated to its own API.

pub mod adapter;
pub mod credential;
mod endpoint;
pub mod sabnzbd;
pub mod settings;
pub mod web;
