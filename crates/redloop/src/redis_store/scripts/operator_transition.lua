local failures = KEYS[1]
local ready = KEYS[2]
local scheduled = KEYS[3]
local leased = KEYS[4]
local lease_meta = KEYS[5]
local failed = KEYS[6]
local workers_leases = KEYS[7]
local rerun = KEYS[8]

local op = ARGV[1]
local job_id = ARGV[2]
local now_us = tonumber(ARGV[3])

local function current_state()
    if redis.call('ZSCORE', ready, job_id) then
        return 'ready'
    end
    if redis.call('ZSCORE', scheduled, job_id) then
        return 'scheduled'
    end
    if redis.call('ZSCORE', leased, job_id) then
        return 'leased'
    end
    if redis.call('ZSCORE', failed, job_id) then
        return 'failed'
    end
    return 'none'
end

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

local state = current_state()
if state == 'none' then
    return {'err', 'not_found'}
end

if op == 'cancel' then
    if state == 'scheduled' then
        redis.call('ZREM', scheduled, job_id)
        redis.call('HDEL', failures, job_id)
        redis.call('SREM', rerun, job_id)
        return {'ok'}
    end
    if state == 'failed' then
        redis.call('ZREM', failed, job_id)
        redis.call('HDEL', failures, job_id)
        redis.call('SREM', rerun, job_id)
        return {'ok'}
    end
    return {'err', 'invalid_state', state}
end

if op == 'requeue' or op == 'retry_now' then
    if state == 'leased' then
        return {'err', 'invalid_state', state}
    end
    if state == 'ready' then
        return {'ok'}
    end
    if state == 'scheduled' then
        redis.call('ZREM', scheduled, job_id)
        redis.call('SREM', rerun, job_id)
        redis.call('ZADD', ready, now_us, job_id)
        return {'ok'}
    end
    redis.call('ZREM', failed, job_id)
    redis.call('HDEL', failures, job_id)
    redis.call('SREM', rerun, job_id)
    redis.call('ZADD', ready, now_us, job_id)
    return {'ok'}
end

if op == 'force_retry' then
    if state ~= 'failed' then
        return {'err', 'invalid_state', state}
    end
    redis.call('ZREM', failed, job_id)
    redis.call('HDEL', failures, job_id)
    redis.call('SREM', rerun, job_id)
    redis.call('ZADD', ready, now_us, job_id)
    return {'ok'}
end

if state ~= 'leased' then
    return {'err', 'invalid_state', state}
end

local meta = redis.call('HGET', lease_meta, job_id)
if not meta then
    return {'err', 'not_found'}
end

local worker_id = worker_from_meta(meta)
if not worker_id then
    return {'err', 'not_found'}
end

redis.call('ZREM', leased, job_id)
redis.call('HDEL', lease_meta, job_id)
decrement_worker(worker_id)

if op == 'force_ack' then
    redis.call('HDEL', failures, job_id)
    if redis.call('SREM', rerun, job_id) == 1 then
        redis.call('ZADD', ready, now_us, job_id)
    end
    return {'ok'}
end

if op == 'force_fail' then
    redis.call('SREM', rerun, job_id)
    redis.call('HINCRBY', failures, job_id, 1)
    redis.call('ZADD', failed, now_us, job_id)
    return {'ok'}
end

return {'err', 'invalid_state', state}
