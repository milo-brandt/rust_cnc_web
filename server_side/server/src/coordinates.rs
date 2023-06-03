use std::{collections::{HashMap, VecDeque}, hash::Hash, sync::Arc, ops::DerefMut};

use axum::{Router, Extension, Json, routing::{get, post, delete}};
use common::api::{Offsets, SetCoordinateOffset, DeleteCoordinateOffset, OffsetKind, SavedPosition};
use serde::{Serialize, Deserialize};

use crate::{util::{file_backed_json::FileBackedValue, exclusive_extension::ExclusiveExtension}, Config, server_result::ServerResult};
use tokio::sync::RwLock;


pub async fn get_service(config: &Config) -> anyhow::Result<Router> {
    let coordinates: FileBackedValue<Offsets> = FileBackedValue::new(
        config.data_folder.join("coordinates/coordinate_offsets.json"), Default::default
    ).await?;
    let positions: FileBackedValue<VecDeque<SavedPosition>> = FileBackedValue::new(
        config.data_folder.join("coordinates/saved_positions.json"), Default::default
    ).await?;
    let router = Router::new()
        .route("/offsets", get(list_offsets).delete(remove_offset).put(set_offset))
        .route("/positions", get(list_positions).post(add_position).layer(ExclusiveExtension::new(positions)))
        .layer(ExclusiveExtension::new(coordinates));
    Ok(router)
}

type CoordinateInfo = ExclusiveExtension<FileBackedValue<Offsets>>;
type PositionInfo = ExclusiveExtension<FileBackedValue<VecDeque<SavedPosition>>>;

async fn list_offsets(coordinate_info: CoordinateInfo) -> Json<Offsets> {
    Json(coordinate_info.read().await.get().clone())
}
async fn set_offset(coordinate_info: CoordinateInfo, input: Json<SetCoordinateOffset>) -> ServerResult<Json<Offsets>> {
    let input = input.0;
    let updated = coordinate_info.write().await.mutate(move |offsets| {
        let map = match input.offset_kind {
            OffsetKind::Tool => &mut offsets.tools,
            OffsetKind::Workpiece => &mut offsets.workpieces,
        };
        map.insert(input.name, input.offset);
        Ok(offsets.clone())
    }).await?;
    Ok(Json(updated))
}
async fn remove_offset(coordinate_info: CoordinateInfo, input: Json<DeleteCoordinateOffset>) -> ServerResult<Json<Offsets>> {
    let input = input.0;
    let updated = coordinate_info.write().await.mutate(move |offsets| {
        let map = match input.offset_kind {
            OffsetKind::Tool => &mut offsets.tools,
            OffsetKind::Workpiece => &mut offsets.workpieces,
        };
        map.remove(&input.name);
        Ok(offsets.clone())
    }).await?;
    Ok(Json(updated))
}

fn get_position_output(positions: &VecDeque<SavedPosition>) -> Json<Vec<SavedPosition>> {
    Json(positions.iter().cloned().collect())
}
async fn list_positions(position_info: PositionInfo) -> Json<Vec<SavedPosition>> {
    get_position_output(position_info.read().await.get())
}
async fn add_position(position_info: PositionInfo, item: Json<SavedPosition>) -> ServerResult<Json<Vec<SavedPosition>>> {
    let mut positions = position_info.write().await;
    Ok(positions.mutate(move |positions| {
        positions.push_back(item.0);
        if positions.len() > 25 {
            positions.pop_front();
        }
        Ok(get_position_output(positions))
    }).await?)
}

/*
Basic interface:

To set up coordinate systems
1. Choose a location in machine coordinates. (Allow displaying it relative to any coordinate system)
2. Choose a tool and a workpiece system; mark one of them to update.
3. Set any coordinates as reference (any selection being optional).
4. Update the tool/workpiece system so that the that machine position would align with the reference in the
   chosen coordinate system.

Then: can also choose a coordinate system by specifying a pair of tool + work position.
* Should be able to create and delete tool/work positions freely (default to no offset - which is probably clearly
  not initialized)

Maybe allow recording positions? Perhaps with names? Perhaps also via probing? Always recorded in machine position - probably
also noting the name of the active tool system, if present.
 */