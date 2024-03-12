-- This file should undo anything in `up.sql`

alter table generation
    add typ text;
