// Created: 2026-10-24
// Reason: Initial public release
// Last updated: 2026-11-10
// Reason: refactor: modularize table column resizing handler from app.js
// Description: Interactive pointer drag-resizing handler for downloads table columns.

"use strict";

function initializeColumnResizing() {
	const table = document.querySelector(".downloads-table");
	const columns = [...document.querySelectorAll(".downloads-table col")];
	const headers = [...document.querySelectorAll(".downloads-table thead th")];
	if (!table || !headers.length) return;

	headers.forEach((header, index) => {
		const handle = document.createElement("span");
		handle.className = "column-resizer";
		handle.setAttribute("aria-hidden", "true");
		header.append(handle);

		handle.addEventListener("pointerdown", (event) => {
			if (event.button !== 0) return;
			event.preventDefault();
			event.stopPropagation();
			const startX = event.clientX,
				startWidth = header.getBoundingClientRect().width,
				startTableWidth = table.getBoundingClientRect().width;
			handle.setPointerCapture(event.pointerId);
			document.body.classList.add("resizing-columns");
			const move = (moveEvent) => {
				const width = Math.max(
						index === 1 ? 20 : 45,
						startWidth + moveEvent.clientX - startX,
					),
					delta = width - startWidth;
				if (columns[index]) {
					columns[index].style.width = `${width}px`;
				}
				table.style.width = `${Math.max(735, startTableWidth + delta)}px`;
				table.style.minWidth = table.style.width;
			};
			const finish = (finishEvent) => {
				try {
					handle.releasePointerCapture(finishEvent.pointerId);
				} catch {}
				handle.removeEventListener("pointermove", move);
				handle.removeEventListener("pointerup", finish);
				handle.removeEventListener("pointercancel", finish);
				document.body.classList.remove("resizing-columns");
			};
			handle.addEventListener("pointermove", move);
			handle.addEventListener("pointerup", finish);
			handle.addEventListener("pointercancel", finish);
		});
	});
}

if (typeof window !== "undefined") {
	window.initializeColumnResizing = initializeColumnResizing;
}

if (typeof module !== "undefined" && module.exports) {
	module.exports = { initializeColumnResizing };
}
