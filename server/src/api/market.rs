//! Market endpoints.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::{
    auth::token::AuthenticatedUser,
    db::{
        DataAccess, DataAccessProvider, Transactable, Transaction,
        model::{
            MarketListing, MarketListingIden, MarketListingInsertion,
            MarketListingStatus, MarketListingType,
        },
    },
    logging::UnexpectedResultExt,
    send_hack::{send_future, send_stream},
};

use super::tracker::UrlEncodedTrackerId;

/// Response for GET /tracker/{tracker_id}/market.
#[derive(Serialize)]
struct GetMarketListingsResponse {
    pub eligible_games: Vec<String>,
    pub listings: Vec<MarketListing>,
}

/// `GET /tracker/{tracker_id}/market`: Get all market listings for a tracker.
pub async fn get_market_listings<D>(
    State(state): State<Arc<crate::state::AppState<D>>>,
    Path(tracker_id): Path<UrlEncodedTrackerId>,
    _user: Option<AuthenticatedUser>,
) -> Result<impl IntoResponse, StatusCode>
where
    D: DataAccessProvider + Send + Sync + 'static,
{
    let mut db = state
        .data_provider
        .create_data_access()
        .await
        .unexpected()?;

    let mut tx = db.begin().await.unexpected()?;

    let tracker = tx
        .get_tracker_by_tracker_id(tracker_id.into())
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    let games: Vec<_> = tx
        .get_ap_games_by_tracker_id(tracker.id)
        .try_collect()
        .await
        .unexpected()?;

    // Compute eligible games: games where more than 1 distinct user has claimed a slot.
    // TODO: Restore the claimed-user filter after testing.  For now, show all
    // games that have at least one slot so the UI can be tested without
    // requiring multiple authenticated users.
    let mut eligible_games: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        for game in &games {
            seen.insert(game.game.clone());
        }
        seen.into_iter().collect()
    };
    eligible_games.sort();

    let listings: Vec<MarketListing> = tx
        .get_market_listings_by_tracker_id(tracker.id)
        .try_collect()
        .await
        .unexpected()?;

    send_future(tx.rollback()).await.unexpected()?;

    Ok(Json(GetMarketListingsResponse {
        eligible_games,
        listings,
    }))
}

/// Request body for creating a market listing.
#[derive(Debug, Deserialize)]
pub struct CreateMarketListingRequest {
    pub ap_game_id: i32,
    pub item_name: String,
    pub listing_type: MarketListingType,
    #[serde(default = "default_quantity")]
    pub quantity: i32,
}

fn default_quantity() -> i32 {
    1
}

/// `POST /tracker/{tracker_id}/market`: Create a new market listing.
pub async fn create_market_listing<D>(
    State(state): State<Arc<crate::state::AppState<D>>>,
    Path(tracker_id): Path<UrlEncodedTrackerId>,
    user: AuthenticatedUser,
    Json(body): Json<CreateMarketListingRequest>,
) -> Result<impl IntoResponse, StatusCode>
where
    D: DataAccessProvider + Send + Sync + 'static,
{
    if body.quantity < 1 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let mut db = state
        .data_provider
        .create_data_access()
        .await
        .unexpected()?;

    let mut tx = db.begin().await.unexpected()?;

    let tracker = tx
        .get_tracker_by_tracker_id(tracker_id.into())
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Verify the game belongs to this tracker and the user has claimed it.
    let game = tx
        .get_ap_game(body.ap_game_id)
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    if game.tracker_id != tracker.id {
        return Err(StatusCode::NOT_FOUND);
    }

    if game.claimed_by_ct_user_id != Some(user.user.id) {
        return Err(StatusCode::FORBIDDEN);
    }

    let listing = MarketListingInsertion {
        ap_tracker_id: tracker.id,
        ct_user_id: user.user.id,
        ap_game_id: game.id,
        game_name: game.game,
        item_name: body.item_name,
        listing_type: body.listing_type,
        quantity: body.quantity,
        status: MarketListingStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created: MarketListing = {
        let stream = send_stream(tx.create_market_listings([listing]));
        tokio::pin!(stream);
        stream
            .try_next()
            .await
            .unexpected()?
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
    };

    send_future(tx.commit()).await.unexpected()?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// Request body for updating a market listing.
#[derive(Debug, Deserialize)]
pub struct UpdateMarketListingRequest {
    #[serde(default)]
    pub status: Option<MarketListingStatus>,
    #[serde(default)]
    pub quantity: Option<i32>,
}

/// `PUT /tracker/{tracker_id}/market/{listing_id}`: Update a market listing.
pub async fn update_market_listing<D>(
    State(state): State<Arc<crate::state::AppState<D>>>,
    Path((tracker_id, listing_id)): Path<(UrlEncodedTrackerId, i32)>,
    user: AuthenticatedUser,
    Json(body): Json<UpdateMarketListingRequest>,
) -> Result<impl IntoResponse, StatusCode>
where
    D: DataAccessProvider + Send + Sync + 'static,
{
    let mut db = state
        .data_provider
        .create_data_access()
        .await
        .unexpected()?;

    let mut tx = db.begin().await.unexpected()?;

    let tracker = tx
        .get_tracker_by_tracker_id(tracker_id.into())
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut listing = tx
        .get_market_listing(listing_id)
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    if listing.ap_tracker_id != tracker.id {
        return Err(StatusCode::NOT_FOUND);
    }

    // Only the listing owner can update it.
    if listing.ct_user_id != user.user.id {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut columns = Vec::new();

    if let Some(status) = body.status {
        listing.status = status;
        columns.push(MarketListingIden::Status);
    }

    if let Some(quantity) = body.quantity {
        if quantity < 1 {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        listing.quantity = quantity;
        columns.push(MarketListingIden::Quantity);
    }

    if columns.is_empty() {
        send_future(tx.rollback()).await.unexpected()?;
        return Ok(Json(listing));
    }

    listing.updated_at = Utc::now();
    columns.push(MarketListingIden::UpdatedAt);

    let listing = tx
        .update_market_listing(listing, &columns)
        .await
        .unexpected()?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    send_future(tx.commit()).await.unexpected()?;

    Ok(Json(listing))
}

/// `DELETE /tracker/{tracker_id}/market/{listing_id}`: Delete a market listing.
pub async fn delete_market_listing<D>(
    State(state): State<Arc<crate::state::AppState<D>>>,
    Path((tracker_id, listing_id)): Path<(UrlEncodedTrackerId, i32)>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, StatusCode>
where
    D: DataAccessProvider + Send + Sync + 'static,
{
    let mut db = state
        .data_provider
        .create_data_access()
        .await
        .unexpected()?;

    let mut tx = db.begin().await.unexpected()?;

    let tracker = tx
        .get_tracker_by_tracker_id(tracker_id.into())
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    let listing = tx
        .get_market_listing(listing_id)
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    if listing.ap_tracker_id != tracker.id {
        return Err(StatusCode::NOT_FOUND);
    }

    // Owner of listing or tracker owner can delete.
    if listing.ct_user_id != user.user.id
        && tracker.owner_ct_user_id != Some(user.user.id)
    {
        return Err(StatusCode::FORBIDDEN);
    }

    tx.delete_market_listing_by_id(listing_id)
        .await
        .unexpected()?;

    send_future(tx.commit()).await.unexpected()?;

    Ok(StatusCode::NO_CONTENT)
}

/// A match between a user's request and another user's offer.
#[derive(Serialize)]
pub struct MarketMatch {
    pub request: MarketListing,
    pub matching_offer: MarketListing,
}

/// `GET /tracker/{tracker_id}/market/matches`: Get matches for the current
/// user's requests.
pub async fn get_market_matches<D>(
    State(state): State<Arc<crate::state::AppState<D>>>,
    Path(tracker_id): Path<UrlEncodedTrackerId>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, StatusCode>
where
    D: DataAccessProvider + Send + Sync + 'static,
{
    let mut db = state
        .data_provider
        .create_data_access()
        .await
        .unexpected()?;

    let mut tx = db.begin().await.unexpected()?;

    let tracker = tx
        .get_tracker_by_tracker_id(tracker_id.into())
        .await
        .unexpected()?
        .ok_or(StatusCode::NOT_FOUND)?;

    let listings: Vec<MarketListing> = tx
        .get_market_listings_by_tracker_id(tracker.id)
        .try_collect()
        .await
        .unexpected()?;

    send_future(tx.rollback()).await.unexpected()?;

    // Find matches: user's active requests that have matching active offers from other users.
    let user_requests: Vec<&MarketListing> = listings
        .iter()
        .filter(|l| {
            l.ct_user_id == user.user.id
                && l.listing_type == MarketListingType::Request
                && l.status == MarketListingStatus::Active
        })
        .collect();

    let other_offers: Vec<&MarketListing> = listings
        .iter()
        .filter(|l| {
            l.ct_user_id != user.user.id
                && l.listing_type == MarketListingType::Offer
                && l.status == MarketListingStatus::Active
        })
        .collect();

    let mut matches = Vec::new();
    for request in &user_requests {
        for offer in &other_offers {
            if offer.game_name == request.game_name && offer.item_name == request.item_name {
                matches.push(MarketMatch {
                    request: (*request).clone(),
                    matching_offer: (*offer).clone(),
                });
            }
        }
    }

    Ok(Json(matches))
}
