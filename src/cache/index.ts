import path from "path"
import fs from "fs/promises"

export default class EventCache {
    private CACHE_DIR = path.join(__dirname, 'data')
    private CACHE_FILE = path.join(this.CACHE_DIR, 'cache.json')

    constructor() {
        fs.mkdir(this.CACHE_DIR, {recursive: true})
    }

    public async readCacheData() {
        return await fs.readFile(this.CACHE_FILE)
    }

    public async writeCacheData(data: string) {
        return await fs.writeFile(this.CACHE_FILE, data)
    }

    public async deleteCacheData() {
        return await fs.unlink(this.CACHE_FILE)
    }
}
