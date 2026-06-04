local leased = KEYS[1]
local lease_meta = KEYS[2]
local ready = KEYS[3]
local workers_leases = KEYS[4]
local rerun = KEYS[5]

local now_us = tonumber(ARGV[1])
local limit = tonumber(ARGV[2])

local function worker_from_meta(meta)
    local delimiter = string.find(meta, ':', 1, true)
    if not delimiter then
        return nil
    end
    local length = tonumber(string.sub(meta, 1, delimiter - 1))
    if not length then
        return nil
    end
    return string.sub(meta, delimiter + 1, delimiter + length)
end

local function decrement_worker(worker_id)
    local count = redis.call('HINCRBY', workers_leases, worker_id, -1)
    if count <= 0 then
        redis.call('HDEL', workers_leases, worker_id)
    end
end

local reaped = 0
for _ = 1, limit do
    local candidate = redis.call('ZRANGEBYSCORE', leased, '-inf', now_us, 'LIMIT', 0, 1)
    if #candidate == 0 then
        break
    end

    local job_id = candidate[1]
    local meta = redis.call('HGET', lease_meta, job_id)
    redis.call('ZREM', leased, job_id)
    redis.call('HDEL', lease_meta, job_id)
    redis.call('SREM', rerun, job_id)
    redis.call('ZADD', ready, now_us, job_id)
    if meta then
        local worker_id = worker_from_meta(meta)
        if worker_id then
            decrement_worker(worker_id)
        end
    end
    reaped = reaped + 1
end

return {'ok', tostring(reaped)}
