local leased = KEYS[1]
local lease_meta = KEYS[2]
local workers_last_seen = KEYS[3]

local job_id = ARGV[1]
local worker_id = ARGV[2]
local lease_token = ARGV[3]
local now_us = tonumber(ARGV[4])
local deadline_us = tonumber(ARGV[5])

local expected_meta = tostring(string.len(worker_id)) .. ':' .. worker_id .. lease_token
local current_meta = redis.call('HGET', lease_meta, job_id)
if not current_meta or current_meta ~= expected_meta or not redis.call('ZSCORE', leased, job_id) then
    return {'err', 'lease_mismatch'}
end

redis.call('ZADD', leased, deadline_us, job_id)
redis.call('ZADD', workers_last_seen, now_us, worker_id)
return {'ok'}
