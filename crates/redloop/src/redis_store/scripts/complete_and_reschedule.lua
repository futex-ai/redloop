local leased = KEYS[1]
local lease_meta = KEYS[2]
local failures = KEYS[3]
local workers_leases = KEYS[4]
local ready = KEYS[5]
local scheduled = KEYS[6]
local rerun = KEYS[7]

local job_id = ARGV[1]
local worker_id = ARGV[2]
local lease_token = ARGV[3]
local now_us = tonumber(ARGV[4])
local schedule_us = tonumber(ARGV[5]) or 0

local expected_meta = tostring(string.len(worker_id)) .. ':' .. worker_id .. lease_token
local current_meta = redis.call('HGET', lease_meta, job_id)
if not current_meta or current_meta ~= expected_meta or not redis.call('ZSCORE', leased, job_id) then
    return {'err', 'lease_mismatch'}
end

redis.call('ZREM', leased, job_id)
redis.call('HDEL', lease_meta, job_id)
redis.call('HDEL', failures, job_id)

local count = redis.call('HINCRBY', workers_leases, worker_id, -1)
if count <= 0 then
    redis.call('HDEL', workers_leases, worker_id)
end

if redis.call('SREM', rerun, job_id) == 1 then
    redis.call('ZADD', ready, now_us, job_id)
    return {'ok', 'ready', '0'}
end

if schedule_us == 0 or schedule_us <= now_us then
    redis.call('ZADD', ready, now_us, job_id)
    return {'ok', 'ready', '0'}
end

redis.call('ZADD', scheduled, schedule_us, job_id)
return {'ok', 'scheduled', string.format('%.0f', schedule_us)}
