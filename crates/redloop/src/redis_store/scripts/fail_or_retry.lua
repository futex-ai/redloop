local leased = KEYS[1]
local lease_meta = KEYS[2]
local failures = KEYS[3]
local ready = KEYS[4]
local scheduled = KEYS[5]
local failed = KEYS[6]
local workers_leases = KEYS[7]
local rerun = KEYS[8]

local job_id = ARGV[1]
local worker_id = ARGV[2]
local lease_token = ARGV[3]
local now_us = tonumber(ARGV[4])
local action_kind = ARGV[5]
local policy_kind = ARGV[6]
local max_retries = tonumber(ARGV[7]) or 0
local backoff_kind = ARGV[8]
local backoff_a = tonumber(ARGV[9]) or 0
local backoff_b = tonumber(ARGV[10]) or 0
local backoff_c = tonumber(ARGV[11]) or 0

local expected_meta = tostring(string.len(worker_id)) .. ':' .. worker_id .. lease_token
local current_meta = redis.call('HGET', lease_meta, job_id)
if not current_meta or current_meta ~= expected_meta or not redis.call('ZSCORE', leased, job_id) then
    return {'err', 'lease_mismatch'}
end

redis.call('ZREM', leased, job_id)
redis.call('HDEL', lease_meta, job_id)
redis.call('SREM', rerun, job_id)

local count = redis.call('HINCRBY', workers_leases, worker_id, -1)
if count <= 0 then
    redis.call('HDEL', workers_leases, worker_id)
end

local failure_count = redis.call('HINCRBY', failures, job_id, 1)
if action_kind == 'terminal' then
    redis.call('ZADD', failed, now_us, job_id)
    return {'ok', tostring(failure_count), 'failed', '0'}
end

local should_retry = true
if policy_kind == 'never' then
    should_retry = false
elseif policy_kind == 'count' and failure_count > max_retries then
    should_retry = false
end

if not should_retry then
    redis.call('ZADD', failed, now_us, job_id)
    return {'ok', tostring(failure_count), 'failed', '0'}
end

if backoff_kind == 'none' then
    redis.call('ZADD', ready, now_us, job_id)
    return {'ok', tostring(failure_count), 'ready', '0'}
end

if backoff_kind == 'fixed' then
    local retry_at = now_us + (backoff_a * 1000)
    if backoff_a == 0 then
        redis.call('ZADD', ready, now_us, job_id)
        return {'ok', tostring(failure_count), 'ready', '0'}
    end
    redis.call('ZADD', scheduled, retry_at, job_id)
    return {'ok', tostring(failure_count), 'scheduled', string.format('%.0f', retry_at)}
end

local delay_ms = backoff_a
for _ = 1, failure_count - 1 do
    delay_ms = math.min(delay_ms * backoff_b, backoff_c)
end

if delay_ms == 0 then
    redis.call('ZADD', ready, now_us, job_id)
    return {'ok', tostring(failure_count), 'ready', '0'}
end

local retry_at = now_us + (delay_ms * 1000)
redis.call('ZADD', scheduled, retry_at, job_id)
return {'ok', tostring(failure_count), 'scheduled', string.format('%.0f', retry_at)}
