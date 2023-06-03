use std::rc::Rc;

use anyhow::Context;
use chrono::Offset;
use common::api::{Offsets, self, OffsetKind, Vec3, SavedPosition};
use sycamore::prelude::*;

use crate::request::{request, HttpMethod, request_with_json};

#[derive(Clone)]
pub struct PositionModel<'a> {
    data: &'a Signal<Vec<SavedPosition>>,
}

impl<'a> PositionModel<'a> {
    pub async fn new(cx: Scope<'a>) -> anyhow::Result<&'a PositionModel<'a>> {
        let initial_data = request(HttpMethod::Get, api::POSITIONS).await.context("getting position data")?;
        Ok(create_ref(cx, PositionModel {
            data: create_signal(cx, initial_data.json().await.context("reading response json")?)
        }))
    }
    pub fn get(&self) -> Rc<Vec<SavedPosition>> {
        self.data.get()
    }
    pub fn signal(&self) -> &ReadSignal<Vec<SavedPosition>> {
        self.data
    }
    pub async fn add(&self, label: String, position: Vec3) -> anyhow::Result<()> {
        self.data.set(request_with_json(
            HttpMethod::Post,
            api::POSITIONS,
            &api::SavedPosition {
                label,
                position
            }
        ).await.context("setting offset data")?.json().await?);
        Ok(())
    }
}