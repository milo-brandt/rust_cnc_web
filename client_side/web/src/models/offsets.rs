use std::rc::Rc;

use anyhow::Context;
use chrono::Offset;
use common::api::{Offsets, self, OffsetKind, Vec3};
use sycamore::prelude::*;

use crate::request::{request, HttpMethod, request_with_json};

#[derive(Clone)]
pub struct OffsetModel<'a> {
    data: &'a Signal<Offsets>,
}

impl<'a> OffsetModel<'a> {
    pub async fn new(cx: Scope<'a>) -> anyhow::Result<&'a OffsetModel<'a>> {
        let initial_data = request(HttpMethod::Get, api::OFFSETS).await.context("getting offset data")?;
        Ok(create_ref(cx, OffsetModel {
            data: create_signal(cx, initial_data.json().await.context("reading response json")?)
        }))
    }
    pub fn get(&self) -> Rc<Offsets> {
        self.data.get()
    }
    pub fn signal(&self) -> &ReadSignal<Offsets> {
        self.data
    }
    pub async fn set(&self, name: String, offset_kind: OffsetKind, offset: Vec3) -> anyhow::Result<()> {
        self.data.set(request_with_json(
            HttpMethod::Put,
            api::OFFSETS,
            &api::SetCoordinateOffset {
                name,
                offset_kind,
                offset,
            }
        ).await.context("setting offset data")?.json().await?);
        Ok(())
    }
    pub async fn delete(&self, name: String, offset_kind: OffsetKind) -> anyhow::Result<()> {
        self.data.set(request_with_json(
            HttpMethod::Delete,
            api::OFFSETS,
            &api::DeleteCoordinateOffset {
                name,
                offset_kind,
            }
        ).await.context("setting offset data")?.json().await?);
        Ok(())
    }
}