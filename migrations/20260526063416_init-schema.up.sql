create table geodata
(
    data jsonb not null,
    geom geometry(Point, 4326),
    ts timestamptz
);

create index idx_geodata_ts on geodata(ts);

create function geodata_compute()
returns trigger as $$
begin
    new.geom := st_point(
        (new.data #>> '{geometry,coordinates,0}')::float,
        (new.data #>> '{geometry,coordinates,1}')::float,
        4326
    );

    new.ts := (new.data #>> '{properties,timestamp}')::timestamptz;
    return new;
end;
$$ language plpgsql;

create trigger trg_geodata_compute before insert on geodata 
for each row execute function geodata_compute();