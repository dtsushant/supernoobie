//! # Recursion I
//!
//! A pulley machine built entirely from complex numbers.
//!
//! The mathematics lives in [`complex`], [`pulley`] and [`dynamics`], none of
//! which depend on anything outside `std`. [`svg`] draws; the `serve` binary
//! (behind the `serve` feature) puts it behind Axum + HTMX so you can poke it
//! while it runs.
//!
//! Reading order: `complex` -> `pulley` -> `dynamics`.

pub mod body3;
pub mod complex;
pub mod dynamics;
pub mod fluid;
pub mod grid;
pub mod pulley;
pub mod quat;
pub mod vec3;
pub mod raster;
pub mod rigid;
pub mod soft;
pub mod svg;


