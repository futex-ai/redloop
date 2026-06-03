local cfg = KEYS[1]
local ready = KEYS[2]
local scheduled = KEYS[3]
local leased = KEYS[4]
local lease_meta = KEYS[5]
local workers_last_seen = KEYS[6]
local workers_config = KEYS[7]
local workers_leases = KEYS[8]

local worker_id = ARGV[1]
local now_us = tonumber(ARGV[2])
local capacity = tonumber(ARGV[3])
local lease_duration_us = tonumber(ARGV[4])
local retry_policy_key = ARGV[5]
local worker_config_record = ARGV[6]

local function pack_meta(token)
    return tostring(string.len(worker_id)) .. ':' .. worker_id .. token
end

local function int_string(value)
    return string.format('%.0f', value)
end

local existing_policy = redis.call('HGET', cfg, 'retry_policy')
if not existing_policy then
    redis.call('HSET', cfg, 'version', '1', 'retry_policy', retry_policy_key)
elseif existing_policy ~= retry_policy_key then
    return {'err', 'invalid_config', 'retry_policy_mismatch'}
end

redis.call('ZADD', workers_last_seen, now_us, worker_id)
redis.call('HSET', workers_config, worker_id, worker_config_record)

local result = {'ok', '0'}
local leased_count = 0

for index = 1, capacity do
    local ready_candidate = redis.call('ZRANGE', ready, 0, 0, 'WITHSCORES')
    local scheduled_candidate = redis.call('ZRANGEBYSCORE', scheduled, '-inf', now_us, 'LIMIT', 0, 1, 'WITHSCORES')

    while #scheduled_candidate > 0 do
        local scheduled_job_id = scheduled_candidate[1]
        local ready_score = redis.call('ZSCORE', ready, scheduled_job_id)
        local leased_score = redis.call('ZSCORE', leased, scheduled_job_id)
        if ready_score or leased_score then
            redis.call('ZREM', scheduled, scheduled_job_id)
            scheduled_candidate = redis.call('ZRANGEBYSCORE', scheduled, '-inf', now_us, 'LIMIT', 0, 1, 'WITHSCORES')
        else
            break
        end
    end

    if #ready_candidate == 0 and #scheduled_candidate == 0 then
        break
    end

    local source = nil
    local job_id = nil
    local score = nil

    if #ready_candidate > 0 and (#scheduled_candidate == 0 or tonumber(ready_candidate[2]) <= tonumber(scheduled_candidate[2])) then
        source = 'ready'
        job_id = ready_candidate[1]
        score = tonumber(ready_candidate[2])
    else
        source = 'scheduled'
        job_id = scheduled_candidate[1]
        score = tonumber(scheduled_candidate[2])
    end

    if source == 'ready' then
        redis.call('ZREM', ready, job_id)
    else
        redis.call('ZREM', scheduled, job_id)
    end

    local token = ARGV[6 + leased_count + 1]
    if not token then
        break
    end

    local deadline = now_us + lease_duration_us
    redis.call('ZADD', leased, deadline, job_id)
    redis.call('HSET', lease_meta, job_id, pack_meta(token))
    redis.call('HINCRBY', workers_leases, worker_id, 1)
    table.insert(result, job_id)
    table.insert(result, token)
    table.insert(result, int_string(deadline))
    leased_count = leased_count + 1
end

local next_future = redis.call('ZRANGEBYSCORE', scheduled, '(' .. now_us, '+inf', 'LIMIT', 0, 1, 'WITHSCORES')
if #next_future > 0 then
    result[2] = next_future[2]
end

return result
