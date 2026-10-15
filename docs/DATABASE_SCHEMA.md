# Database Schema

## wallpapers

id
provider
provider_id
title
image_url
thumbnail_url
local_path
width
height
created_at

## collections

id
name
created_at

## collection_wallpapers

collection_id
wallpaper_id

## favorites

id
wallpaper_id
created_at

## wallpaper_history

id
wallpaper_id
applied_at

## rotation_settings

id
collection_id
interval_minutes
rotation_mode
enabled
