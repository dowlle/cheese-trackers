<script setup>
import { ref, computed, onMounted, watch } from 'vue';
import { filter, groupBy, orderBy } from 'lodash-es';

import { settings, currentUser } from '@/settings';
import { getMarketListings as apiGetMarketListings, createMarketListing as apiCreateMarketListing, updateMarketListing as apiUpdateMarketListing, deleteMarketListing as apiDeleteMarketListing } from '@/api';
import { marketListingType, marketListingStatus } from '@/types';
import MarketGameTab from '@/components/MarketGameTab.vue';

const props = defineProps(['aptrackerid', 'trackerData', 'gameById']);

const emit = defineEmits(['matchCount']);

const loading = ref(false);
const error = ref(undefined);
const eligibleGames = ref([]);
const listings = ref([]);
const selectedGame = ref(undefined);

const listingsByGame = computed(() => groupBy(listings.value, 'game_name'));

const userGames = computed(() => {
    if (!currentUser.value?.id || !props.trackerData?.games) return [];
    return filter(props.trackerData.games, g => g.claimed_by_ct_user_id === currentUser.value.id);
});

const userGameForSelectedGame = computed(() => {
    if (!selectedGame.value) return undefined;
    return userGames.value.find(g => g.game === selectedGame.value);
});

async function loadListings() {
    if (loading.value) return;
    loading.value = true;
    error.value = undefined;

    try {
        const { data } = await apiGetMarketListings(props.aptrackerid);
        eligibleGames.value = data.eligible_games;
        listings.value = data.listings;

        if (!selectedGame.value && eligibleGames.value.length > 0) {
            selectedGame.value = eligibleGames.value[0];
        }

        computeMatchCount();
    } catch (e) {
        error.value = e;
    } finally {
        loading.value = false;
    }
}

function computeMatchCount() {
    if (!currentUser.value?.id) {
        emit('matchCount', 0);
        return;
    }

    const userRequests = filter(listings.value, l =>
        l.ct_user_id === currentUser.value.id &&
        l.listing_type === 'request' &&
        l.status === 'active'
    );

    const otherOffers = filter(listings.value, l =>
        l.ct_user_id !== currentUser.value.id &&
        l.listing_type === 'offer' &&
        l.status === 'active'
    );

    let count = 0;
    for (const req of userRequests) {
        for (const offer of otherOffers) {
            if (offer.game_name === req.game_name && offer.item_name === req.item_name) {
                count++;
                break;
            }
        }
    }

    emit('matchCount', count);
}

async function createListing(data) {
    loading.value = true;
    try {
        await apiCreateMarketListing(props.aptrackerid, data);
        await loadListings();
    } catch (e) {
        error.value = e;
        loading.value = false;
    }
}

async function updateListing(listingId, data) {
    loading.value = true;
    try {
        await apiUpdateMarketListing(props.aptrackerid, listingId, data);
        await loadListings();
    } catch (e) {
        error.value = e;
        loading.value = false;
    }
}

async function deleteListing(listingId) {
    loading.value = true;
    try {
        await apiDeleteMarketListing(props.aptrackerid, listingId);
        await loadListings();
    } catch (e) {
        error.value = e;
        loading.value = false;
    }
}

onMounted(loadListings);
</script>

<template>
    <div class="container bg-dark-subtle pt-3 pb-3 mb-4 rounded">
        <div class="d-flex justify-content-between align-items-center mb-3">
            <h4 class="mb-0"><i class="bi-shop"/> Market</h4>
            <button class="btn btn-sm btn-outline-light" :disabled="loading" @click="loadListings">
                <i class="bi-arrow-clockwise" :class="{ 'spin': loading }"/>
            </button>
        </div>

        <div v-if="error" class="alert alert-danger">
            Failed to load market data.
        </div>

        <div v-if="eligibleGames.length === 0 && !loading && !error" class="text-center text-muted">
            No eligible games yet. A game needs at least 2 players with claimed slots to enable the market.
        </div>

        <template v-if="eligibleGames.length > 0">
            <ul class="nav nav-pills mb-3">
                <li class="nav-item" v-for="game in eligibleGames" :key="game">
                    <button
                        class="nav-link"
                        :class="{ active: selectedGame === game }"
                        @click="selectedGame = game"
                    >
                        {{ game }}
                        <span v-if="(listingsByGame[game] || []).length" class="badge bg-secondary ms-1">
                            {{ (listingsByGame[game] || []).length }}
                        </span>
                    </button>
                </li>
            </ul>

            <MarketGameTab
                v-if="selectedGame"
                :game="selectedGame"
                :listings="listingsByGame[selectedGame] || []"
                :userGame="userGameForSelectedGame"
                :currentUserId="currentUser?.id"
                :loading="loading"
                :aptrackerid="aptrackerid"
                :trackerData="trackerData"
                @create="createListing"
                @update="(id, data) => updateListing(id, data)"
                @delete="deleteListing"
            />
        </template>
    </div>
</template>

<style scoped>
.spin {
    animation: spin 1s linear infinite;
}
@keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}
</style>
