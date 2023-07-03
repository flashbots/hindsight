import Matchmaker, {EventHistoryEntry} from '@flashbots/matchmaker-ts'
import { TransactionResponse, Wallet } from 'ethers'
import Config from './config'
import { EthProvider, EthProviderWs } from './provider'

export class Hindsight {
    public readonly matchmaker: Matchmaker
    public readonly authSigner: Wallet
    public readonly provider: EthProvider | EthProviderWs
    private static NUM_BLOCKS = process.env.NODE_ENV === "production" ? 256 : 10

    constructor(matchmaker: Matchmaker, authSigner: Wallet, provider: EthProvider | EthProviderWs) {
        this.matchmaker = matchmaker
        this.authSigner = authSigner
        this.provider = provider
    }

    public async getMevShareHistory() {
        const eventInfo = await this.matchmaker.getEventHistoryInfo()
        const latestBlock = await this.provider.getBlockNumber()
        console.log("latest block", latestBlock)

        let done = false
        let i = 0
        let events: Array<EventHistoryEntry> = []
        console.log(`fetching events from last ${Hindsight.NUM_BLOCKS} blocks`)
        while (!done) {
            const mevShareHistory = await this.matchmaker.getEventHistory({
                blockStart: latestBlock - Hindsight.NUM_BLOCKS,
                limit: eventInfo.maxLimit,
                offset: i * eventInfo.maxLimit,
            })
            i++
            if (mevShareHistory.length < eventInfo.maxLimit) {
                done = true
            }
            events.push(...mevShareHistory)
            console.log(`accumulated ${events.length} events`)
        }

        return events
    }

    public async filterEvents(events: Array<EventHistoryEntry>, targetTopics: Array<string>) {
        const eventHashMap = new Map<string, number>()
        for (const topic of targetTopics) {
            eventHashMap.set(topic, 0)
        }
        let eligibleEvents = []
        for (const event of events) {
            for (const log of event.hint.logs || []) {
                // track incidence of all log topics for inspiration
                eventHashMap.set(log.topics[0], (eventHashMap.get(log.topics[0]) || 0) + 1)
                if (targetTopics.includes(log.topics[0])) {
                    eligibleEvents.push(event)
                }
            }
        }
        console.log(eventHashMap.entries())
        return eligibleEvents
    }

    public async fetchTxs(events: Array<EventHistoryEntry>) {
        let eligibleTxs: TransactionResponse[] = []
        for (const event of events) {
            try {
                const tx = await this.provider.getTransaction(event.hint.hash)
                if (tx) eligibleTxs.push(tx)
            } catch (e) {
                console.log(`error getting transaction from hash: ${event.hint.hash}`, e)
            }
        }
        return eligibleTxs
    }

    public destroy() {
        return this.provider.destroy()
    }
}

export default class HindsightFactory {
    public matchmaker?: Matchmaker
    public readonly authSigner: Wallet
    public readonly provider: EthProvider | EthProviderWs

    /**
     * @constructor Create a new Hindsight instance, connect providers
     */
    constructor() {
        console.log('hindsight')
        const env = new Config()
        this.authSigner = new Wallet(env.AUTH_SIGNER_PRIVATE_KEY)
        if (env.RPC_URL_WS) {
            this.provider = new EthProviderWs()
        } else {
            this.provider = new EthProvider()
        }
    }

    /**
     * @method init Connect to mev-share using authSigner and provider from constructor. Return a new Hindsight instance.
    */
    public async init(): Promise<Hindsight> {
        this.matchmaker = Matchmaker.fromNetwork(this.authSigner, await this.provider.getNetwork())
        return new Hindsight(this.matchmaker, this.authSigner, this.provider)
    }
}
