import { expect, assert } from "chai"

import Hindsight from "../hindsight"
import { JsonRpcProvider } from 'ethers'

describe("Flashbots Simulation", () => {
    const fbProvider = new JsonRpcProvider("https://rpc.flashbots.net")
    const hindsightPromise = new Hindsight().init()

    it("should get an eth balance for the past 256 blocks", async () => {
        const hindsight = await hindsightPromise
        const startBlock = await hindsight.provider.getBlockNumber()
        for (let i = 0; i < 256; i++) {
            // TODO: make a bundle that includes a balance call to the zero address
            
            const balance = await hindsight.provider.getBalance("0x0000000000000000000000000000000000000000", startBlock - i)
            console.log(balance)
            // hindsight.matchmaker.
            assert(balance > 0n)
        }
        return true
    })

    after(async () => {
        await fbProvider.destroy();
        (await hindsightPromise).destroy()
    })
})
