import Hindsight from './hindsight'
import {stringify as stringifyJson, parse as parseJson} from "json-bigint"
import EventCache from './cache'

async function main() {
    const hindsight = await new Hindsight().init()
    const cache = new EventCache()
    console.log('hindsight', hindsight)

    let skipDownload = false
    let cachedEvents: any[] = []
    try {
        const cacheFile = await cache.readCacheData()
        skipDownload = true
        cachedEvents = parseJson((await cacheFile).toString())
    } catch (e) {
        console.log("no cache file")
    }
    
    // load events
    const events = skipDownload ? cachedEvents : await hindsight.getMevShareHistory()
    events.forEach(event => {
        console.log("logs", event.hint.logs)
        console.log("txs", event.hint.txs)
    })
    console.log("total events", events.length)

    // filter down to events with uniswap hints
    // const eventsWithUniswapHints = events.filter((event) => event.hint.logs && event.hint.logs.includes)

    // TODO: get raw txs of remaining events

    if (!skipDownload) {
        // write new cache file
        await cache.writeCacheData(stringifyJson(events, null, 2))
    }
}

main().then(() => {
    process.exit(0)
})
