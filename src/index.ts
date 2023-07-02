import Hindsight from './hindsight'

async function main() {
    const hindsight = await new Hindsight().init()
    console.log('hindsight', hindsight)
    const mevShareHistory = await hindsight.getMevShareHistory()
    console.log("mevShareHistory", mevShareHistory)
}

main().then(() => {
    process.exit(0)
})
