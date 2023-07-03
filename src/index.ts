import Hindsight from './hindsight'
import {stringify as stringifyJson, parse as parseJson} from "json-bigint"
import EventCache from './cache'
import { EventHistoryEntry } from '@flashbots/matchmaker-ts'
import { TransactionResponseParams, Signature } from 'ethers'

const parseArgs = () => {
    const args = process.argv.slice(2)
    const argMap = {
        deleteCache: args.includes('clean') || args.includes('delete'),
    }

    return argMap
}

async function main() {
    const args = parseArgs()
    if (args.deleteCache) {
        console.log("deleting cache data")
        const cache = new EventCache()
        await cache.deleteCacheData()
        return
    }

    const hindsight = await new Hindsight().init()
    const cache = new EventCache()
    console.log('hindsight', hindsight)

    let skipDownload = false
    let cachedEvents: any[] = []
    let cachedTxs: any[] = []
    try {
        const cacheFile = await cache.readCacheData()
        console.log("loaded cached data")
        skipDownload = true
        const cacheData = parseJson((await cacheFile).toString())
        cachedEvents = cacheData.events
        cachedTxs = cacheData.transactions
    } catch (e) {
        console.log("no cache file")
    }
    
    // load events
    const events: Array<EventHistoryEntry> = skipDownload ? cachedEvents : await hindsight.getMevShareHistory()
    console.log("total events", events.length)

    let eligibleEvents: EventHistoryEntry[] = cachedEvents
    let eligibleTxs: TransactionResponseParams[] = cachedTxs

    if (!skipDownload) {
        // find uniswap-related events
        const uniswapTopics = [
            // univ3
            // Swap(address,address,int256,int256,uint160,uint128,int24)
            "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
            // univ2
            // Sync(uint112,uint112)
            "0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1",
            // univ2
            // Swap(address,uint256,uint256,uint256,uint256,address)
            "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822",
        ]
        eligibleEvents = await hindsight.filterEvents(events, uniswapTopics)

        // get raw txs of eligible events
        eligibleTxs = await hindsight.filterTxs(eligibleEvents)
    }

    // ... doing stuff with the transactions ...
    console.log("eligible txs", Signature.from(eligibleTxs[0].signature).serialized)
    // TODO: add simulations to Hindsight, then invoke that here

    if (!skipDownload) {
        // write new cache file
        await cache.writeCacheData(stringifyJson({
            events: eligibleEvents,
            transactions: eligibleTxs,
        }, null, 2))
    }
}

main().then(() => {
    process.exit(0)
})
