import Hindsight, { TransactionResponseParamsSerialized } from './hindsight'
import {stringify as stringifyJson, parse as parseJson} from "json-bigint"
import EventCache from './cache'
import { EventHistoryEntry } from '@flashbots/matchmaker-ts'
import { Signature } from 'ethers'

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
    try {
        const cacheFile = await cache.readCacheData()
        console.log("loaded cached data")
        skipDownload = true
        const cacheData = parseJson((await cacheFile).toString())
        cachedEvents = cacheData.events
    } catch (e) {
        console.log("no cache file")
    }
    
    // load events
    const events: Array<EventHistoryEntry> = skipDownload ? cachedEvents : await hindsight.getMevShareHistory()
    console.log("total events", events.length)

    let eligibleEvents: EventHistoryEntry[] = cachedEvents

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

    if (!skipDownload) {
        // find uniswap-related events
        eligibleEvents = await hindsight.filterEvents(events, uniswapTopics)
    }

    if (!skipDownload) {
        // write new cache file
        await cache.writeCacheData(stringifyJson({
            events: eligibleEvents,
        }, null, 2))
    }
}

main().then(() => {
    process.exit(0)
})
