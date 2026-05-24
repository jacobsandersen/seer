create table record (
  data text not null,
  lat generated always as (json_extract(data, "$.geometry.coordinates[0]")) stored,
  lon generated always as (json_extract(data, "$.geometry.coordinates[1]")) stored,
  timestamp generated always as (json_extract(data, "$.properties.timestamp")) stored
);

create index idx_timestamp on record(timestamp);