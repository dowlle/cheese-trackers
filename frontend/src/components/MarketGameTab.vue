<script setup>
import { ref, computed } from 'vue';
import { filter, orderBy } from 'lodash-es';

import { marketListingType, marketListingStatus } from '@/types';
import UsernameDisplay from '@/components/UsernameDisplay.vue';
import { getClaimingUserForGame } from '@/types';

const props = defineProps(['game', 'listings', 'userGame', 'currentUserId', 'loading', 'aptrackerid', 'trackerData']);
const emit = defineEmits(['create', 'update', 'delete']);

const newItemName = ref('');
const newListingType = ref('request');
const newQuantity = ref(1);

const activeOffers = computed(() =>
    orderBy(
        filter(props.listings, l => l.listing_type === 'offer' && l.status === 'active'),
        'created_at', 'desc'
    )
);

const activeRequests = computed(() =>
    orderBy(
        filter(props.listings, l => l.listing_type === 'request' && l.status === 'active'),
        'created_at', 'desc'
    )
);

const fulfilledListings = computed(() =>
    filter(props.listings, l => l.status !== 'active')
);

// Check if an item has a match (offer matches a request from another user)
function hasMatch(listing) {
    if (!props.currentUserId) return false;

    if (listing.listing_type === 'request' && listing.ct_user_id === props.currentUserId) {
        return activeOffers.value.some(o =>
            o.item_name === listing.item_name && o.ct_user_id !== props.currentUserId
        );
    }
    return false;
}

function getUserForListing(listing) {
    const game = props.trackerData?.games?.find(g => g.id === listing.ap_game_id);
    return game ? getClaimingUserForGame(game) : undefined;
}

function submitListing() {
    if (!newItemName.value.trim() || !props.userGame) return;

    emit('create', {
        ap_game_id: props.userGame.id,
        item_name: newItemName.value.trim(),
        listing_type: newListingType.value,
        quantity: newQuantity.value,
    });

    newItemName.value = '';
    newQuantity.value = 1;
}
</script>

<template>
    <div>
        <!-- Create listing form -->
        <div v-if="userGame" class="card bg-dark mb-3">
            <div class="card-body">
                <h6 class="card-title">Post a listing</h6>
                <form @submit.prevent="submitListing" class="row g-2 align-items-end">
                    <div class="col-auto">
                        <label class="form-label small">Type</label>
                        <select class="form-select form-select-sm" v-model="newListingType">
                            <option v-for="t in marketListingType" :key="t.id" :value="t.id">
                                {{ t.label }}
                            </option>
                        </select>
                    </div>
                    <div class="col">
                        <label class="form-label small">Item</label>
                        <input
                            class="form-control form-control-sm"
                            v-model="newItemName"
                            placeholder="Item name"
                            list="market-items"
                        >
                    </div>
                    <div class="col-auto" style="width: 80px">
                        <label class="form-label small">Qty</label>
                        <input
                            type="number"
                            class="form-control form-control-sm"
                            v-model.number="newQuantity"
                            min="1"
                        >
                    </div>
                    <div class="col-auto">
                        <button
                            type="submit"
                            class="btn btn-sm btn-primary"
                            :disabled="loading || !newItemName.trim()"
                        >
                            Post
                        </button>
                    </div>
                </form>
            </div>
        </div>
        <div v-else class="alert alert-info">
            You need to claim a {{ game }} slot to post market listings.
        </div>

        <!-- Offers and Requests side by side -->
        <div class="row">
            <div class="col-md-6">
                <h5><span class="badge bg-success"><i class="bi-box-arrow-up"/> Offers</span></h5>
                <div v-if="activeOffers.length === 0" class="text-muted small">No active offers.</div>
                <div v-for="listing in activeOffers" :key="listing.id" class="card bg-dark mb-2">
                    <div class="card-body py-2 px-3">
                        <div class="d-flex justify-content-between align-items-center">
                            <div>
                                <strong>{{ listing.item_name }}</strong>
                                <span v-if="listing.quantity > 1" class="text-muted ms-1">x{{ listing.quantity }}</span>
                                <br>
                                <small class="text-muted">
                                    by <UsernameDisplay :user="getUserForListing(listing)" size="sm"/>
                                </small>
                            </div>
                            <div v-if="listing.ct_user_id === currentUserId" class="btn-group btn-group-sm">
                                <button
                                    class="btn btn-outline-success btn-sm"
                                    title="Mark fulfilled"
                                    @click="emit('update', listing.id, { status: 'fulfilled' })"
                                    :disabled="loading"
                                >
                                    <i class="bi-check-lg"/>
                                </button>
                                <button
                                    class="btn btn-outline-danger btn-sm"
                                    title="Delete"
                                    @click="emit('delete', listing.id)"
                                    :disabled="loading"
                                >
                                    <i class="bi-trash"/>
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="col-md-6">
                <h5><span class="badge bg-warning text-dark"><i class="bi-box-arrow-in-down"/> Requests</span></h5>
                <div v-if="activeRequests.length === 0" class="text-muted small">No active requests.</div>
                <div
                    v-for="listing in activeRequests"
                    :key="listing.id"
                    class="card mb-2"
                    :class="hasMatch(listing) ? 'bg-success bg-opacity-25 border-success' : 'bg-dark'"
                >
                    <div class="card-body py-2 px-3">
                        <div class="d-flex justify-content-between align-items-center">
                            <div>
                                <strong>{{ listing.item_name }}</strong>
                                <span v-if="listing.quantity > 1" class="text-muted ms-1">x{{ listing.quantity }}</span>
                                <span v-if="hasMatch(listing)" class="badge bg-success ms-2">
                                    <i class="bi-check-circle"/> Match found!
                                </span>
                                <br>
                                <small class="text-muted">
                                    by <UsernameDisplay :user="getUserForListing(listing)" size="sm"/>
                                </small>
                            </div>
                            <div v-if="listing.ct_user_id === currentUserId" class="btn-group btn-group-sm">
                                <button
                                    class="btn btn-outline-success btn-sm"
                                    title="Mark fulfilled"
                                    @click="emit('update', listing.id, { status: 'fulfilled' })"
                                    :disabled="loading"
                                >
                                    <i class="bi-check-lg"/>
                                </button>
                                <button
                                    class="btn btn-outline-danger btn-sm"
                                    title="Delete"
                                    @click="emit('delete', listing.id)"
                                    :disabled="loading"
                                >
                                    <i class="bi-trash"/>
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Fulfilled/cancelled listings (collapsed) -->
        <details v-if="fulfilledListings.length > 0" class="mt-3">
            <summary class="text-muted small">{{ fulfilledListings.length }} completed/cancelled listing(s)</summary>
            <div v-for="listing in fulfilledListings" :key="listing.id" class="card bg-dark mb-1 mt-1 opacity-50">
                <div class="card-body py-1 px-3">
                    <small>
                        <span :class="`badge bg-${marketListingType.byId[listing.listing_type]?.color}`">
                            {{ marketListingType.byId[listing.listing_type]?.label }}
                        </span>
                        {{ listing.item_name }}
                        <span v-if="listing.quantity > 1">x{{ listing.quantity }}</span>
                        &mdash;
                        <span :class="`text-${marketListingStatus.byId[listing.status]?.color}`">
                            {{ marketListingStatus.byId[listing.status]?.label }}
                        </span>
                    </small>
                </div>
            </div>
        </details>
    </div>
</template>
