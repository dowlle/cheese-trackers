<script setup>
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { filter, orderBy, flatMap } from 'lodash-es';
import axios from 'axios';

import { marketListingType, marketListingStatus } from '@/types';
import UsernameDisplay from '@/components/UsernameDisplay.vue';
import { getClaimingUserForGame } from '@/types';

const props = defineProps(['game', 'listings', 'userGame', 'currentUserId', 'loading', 'aptrackerid', 'trackerData']);
const emit = defineEmits(['create', 'update', 'delete']);

const newItemName = ref('');
const newListingType = ref('request');
const newQuantity = ref(1);

// Catalog data
const catalogCategories = ref([]);
const catalogLoading = ref(false);
const showDropdown = ref(false);
const searchQuery = ref('');
const highlightIndex = ref(-1);
const itemInputRef = ref(null);
const dropdownRef = ref(null);

// Game name to catalog filename mapping
const CATALOG_GAMES = {
    'Satisfactory': 'satisfactory.json',
    'Stardew Valley': 'stardew_valley.json',
};

async function loadCatalog() {
    const filename = CATALOG_GAMES[props.game];
    if (!filename) return;

    catalogLoading.value = true;
    try {
        const { data } = await axios.get(`/catalog/games/${filename}`);
        catalogCategories.value = data.categories || [];
    } catch (e) {
        console.log('Could not load item catalog for', props.game, e);
        catalogCategories.value = [];
    } finally {
        catalogLoading.value = false;
    }
}

// All items flattened with category info
const allItems = computed(() =>
    flatMap(catalogCategories.value, cat =>
        cat.items.map(item => ({ ...item, category: cat.name }))
    )
);

// Filtered items based on search
const filteredItems = computed(() => {
    const q = searchQuery.value.toLowerCase().trim();
    if (!q) return allItems.value;
    return filter(allItems.value, item =>
        item.name.toLowerCase().includes(q)
    );
});

// Group filtered items by category for display
const filteredByCategory = computed(() => {
    const groups = [];
    let currentCat = null;
    for (const item of filteredItems.value) {
        if (item.category !== currentCat) {
            currentCat = item.category;
            groups.push({ name: currentCat, items: [] });
        }
        groups[groups.length - 1].items.push(item);
    }
    return groups;
});

function selectItem(item) {
    newItemName.value = item.name;
    searchQuery.value = '';
    showDropdown.value = false;
    highlightIndex.value = -1;
}

function onInputFocus() {
    searchQuery.value = '';
    showDropdown.value = true;
    highlightIndex.value = -1;
}

function onInputBlur() {
    // Delay to allow click on dropdown item
    setTimeout(() => {
        showDropdown.value = false;
    }, 200);
}

function onInputChange(e) {
    searchQuery.value = e.target.value;
    newItemName.value = e.target.value;
    showDropdown.value = true;
    highlightIndex.value = -1;
}

function onKeydown(e) {
    if (!showDropdown.value) return;

    const items = filteredItems.value;
    if (e.key === 'ArrowDown') {
        e.preventDefault();
        highlightIndex.value = Math.min(highlightIndex.value + 1, items.length - 1);
        scrollToHighlighted();
    } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        highlightIndex.value = Math.max(highlightIndex.value - 1, 0);
        scrollToHighlighted();
    } else if (e.key === 'Enter' && highlightIndex.value >= 0 && highlightIndex.value < items.length) {
        e.preventDefault();
        selectItem(items[highlightIndex.value]);
    } else if (e.key === 'Escape') {
        showDropdown.value = false;
    }
}

function scrollToHighlighted() {
    nextTick(() => {
        const el = dropdownRef.value?.querySelector('.dropdown-item.active');
        if (el) el.scrollIntoView({ block: 'nearest' });
    });
}

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
    searchQuery.value = '';
    newQuantity.value = 1;
}

// Track flat index for keyboard nav
function flatIndex(catIdx, itemIdx) {
    let idx = 0;
    for (let c = 0; c < catIdx; c++) {
        idx += filteredByCategory.value[c].items.length;
    }
    return idx + itemIdx;
}

onMounted(loadCatalog);
watch(() => props.game, loadCatalog);
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
                    <div class="col position-relative">
                        <label class="form-label small">Item</label>
                        <input
                            ref="itemInputRef"
                            class="form-control form-control-sm"
                            :value="newItemName"
                            @input="onInputChange"
                            @focus="onInputFocus"
                            @blur="onInputBlur"
                            @keydown="onKeydown"
                            placeholder="Search items..."
                            autocomplete="off"
                        >
                        <div
                            v-if="showDropdown && (filteredItems.length > 0 || catalogLoading)"
                            ref="dropdownRef"
                            class="dropdown-menu show w-100 item-dropdown"
                        >
                            <div v-if="catalogLoading" class="dropdown-item text-muted">Loading catalog...</div>
                            <template v-else>
                                <template v-for="(cat, catIdx) in filteredByCategory" :key="cat.name">
                                    <h6 class="dropdown-header">{{ cat.name }}</h6>
                                    <button
                                        v-for="(item, itemIdx) in cat.items"
                                        :key="item.name"
                                        type="button"
                                        class="dropdown-item"
                                        :class="{ active: highlightIndex === flatIndex(catIdx, itemIdx) }"
                                        @mousedown.prevent="selectItem(item)"
                                    >
                                        {{ item.name }}
                                    </button>
                                </template>
                            </template>
                        </div>
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
                                    by <UsernameDisplay :user="getUserForListing(listing)"/>
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
                                    by <UsernameDisplay :user="getUserForListing(listing)"/>
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

<style scoped>
.item-dropdown {
    max-height: 300px;
    overflow-y: auto;
    z-index: 1050;
}
.item-dropdown .dropdown-header {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #adb5bd;
    padding: 0.25rem 1rem;
    margin-top: 0.25rem;
}
.item-dropdown .dropdown-item {
    font-size: 0.85rem;
    padding: 0.25rem 1rem;
}
.item-dropdown .dropdown-item.active {
    background-color: var(--bs-primary);
}
</style>
