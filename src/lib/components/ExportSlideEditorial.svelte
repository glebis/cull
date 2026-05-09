<script lang="ts">
	import type { Slide, ManifestDefaults, ExportTarget } from '$lib/export-types';

	let { slide, defaults, target, imageSrc }: {
		slide: Slide;
		defaults: ManifestDefaults;
		target: ExportTarget;
		imageSrc: string;
	} = $props();

	let focalX = $derived(slide.image.focal_point ? `${slide.image.focal_point.x * 100}%` : '50%');
	let focalY = $derived(slide.image.focal_point ? `${slide.image.focal_point.y * 100}%` : '50%');
</script>

<div
	class="editorial-slide"
	style:width="{target.width}px"
	style:height="{target.height}px"
	style:background={defaults.colors.background}
	style:color={defaults.colors.foreground}
	style:padding="{defaults.safe_area.top}px {defaults.safe_area.right}px {defaults.safe_area.bottom}px {defaults.safe_area.left}px"
>
	{#if slide.text.headline}
		<h1 class="headline">{slide.text.headline}</h1>
	{/if}

	<div class="divider" style:background={defaults.colors.accent}></div>

	<div class="image-frame" style:border-color="{defaults.colors.accent}33">
		<img
			src={imageSrc}
			alt={slide.metadata.alt || ''}
			style:object-position="{focalX} {focalY}"
		/>
	</div>

	{#if slide.text.body}
		<p class="body">{slide.text.body}</p>
	{/if}

	{#if slide.text.caption}
		<span class="caption">{slide.text.caption}</span>
	{/if}
</div>

<style>
	.editorial-slide {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 20px;
		box-sizing: border-box;
	}

	.headline {
		font-family: var(--font-serif);
		font-size: 56px;
		font-weight: 700;
		line-height: 1.15;
		text-align: center;
		margin: 0;
		max-width: 90%;
	}

	.divider {
		width: 60px;
		height: 2px;
		flex-shrink: 0;
		margin: 4px 0;
	}

	.image-frame {
		max-width: 80%;
		max-height: 50%;
		overflow: hidden;
		border: 1px solid;
		flex-shrink: 1;
		min-height: 0;
	}

	.image-frame img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.body {
		font-family: var(--font-serif);
		font-size: 28px;
		font-weight: 400;
		line-height: 1.6;
		text-align: center;
		margin: 0;
		max-width: 85%;
	}

	.caption {
		font-family: var(--font);
		font-size: 16px;
		font-weight: 400;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		opacity: 0.5;
		text-align: center;
	}
</style>
