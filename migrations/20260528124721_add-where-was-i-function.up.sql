create function where_was_i(wanted_ts timestamptz) 
returns table(data jsonb, geom geometry, recorded_at timestamptz)
language plpgsql stable
as $$
begin
  return query

  with nearest as (
    (select * from geodata
      where ts <= wanted_ts
      order by ts desc 
      limit 1)

    union all

    (select * from geodata
      where ts >= wanted_ts
      order by ts asc 
      limit 1)
  )
  
  select * from nearest
  order by abs(extract(epoch from (ts - wanted_ts)))
  limit 1;
end;
$$;

create function where_was_i_between(start_ts timestamptz, end_ts timestamptz)
returns table(data jsonb, geom geometry, recorded_at timestamptz)
language plpgsql stable
as $$
begin
  return query
  select * from geodata
    where ts >= start_ts
      and ts < end_ts
    order by ts asc;

  if not found then
    return query
    select * from geodata
      where ts < start_ts
      order by ts desc
      limit 1;
  end if;
end;
$$;