// https://github.com/gpac/mp4box.js/issues/233

import { Flags } from "./utils/tauri";

declare module "mp4box";
export interface MP4MediaTrack {
	id: number;
	created: Date;
	modified: Date;
	movie_duration: number;
	layer: number;
	alternate_group: number;
	volume: number;
	track_width: number;
	track_height: number;
	timescale: number;
	duration: number;
	bitrate: number;
	codec: string;
	language: string;
	nb_samples: number;
}

export interface MP4VideoData {
	width: number;
	height: number;
}

export interface MP4VideoTrack extends MP4MediaTrack {
	video: MP4VideoData;
}

export interface MP4AudioData {
	sample_rate: number;
	channel_count: number;
	sample_size: number;
}

export interface MP4AudioTrack extends MP4MediaTrack {
	audio: MP4AudioData;
}

export type MP4Track = MP4VideoTrack | MP4AudioTrack;

export interface MP4Info {
	duration: number;
	timescale: number;
	fragment_duration: number;
	isFragmented: boolean;
	isProgressive: boolean;
	hasIOD: boolean;
	brands: string[];
	created: Date;
	modified: Date;
	tracks: MP4Track[];
	mime: string;
	audioTracks: MP4AudioTrack[];
	videoTracks: MP4VideoTrack[];
}

export type MP4ArrayBuffer = ArrayBuffer & { fileStart: number };

export interface MP4File {
	onMoovStart?: () => void;
	onReady?: (info: MP4Info) => void;
	onError?: (e: string) => void;
	onSamples?: (id: number, user: unknown, samples: Sample[]) => void;

	appendBuffer(data: MP4ArrayBuffer): number;
	start(): void;
	stop(): void;
	flush(): void;

	setExtractionOptions(
		id: number,
		user: unknown,
		options: ExtractionOptions,
	): void;
}

export function createFile(): MP4File;

export interface Sample {
	number: number;
	track_id: number;
	timescale: number;
	description_index: number;
	description: {
		avcC?: BoxParser.avcCBox; // h.264
		hvcC?: BoxParser.hvcCBox; // hevc
		vpcC?: BoxParser.vpcCBox; // vp9
		av1C?: BoxParser.av1CBox; // av1
	};
	data: ArrayBuffer;
	size: number;
	alreadyRead?: number;
	duration: number;
	cts: number;
	dts: number;
	is_sync: boolean;
	is_leading?: number;
	depends_on?: number;
	is_depended_on?: number;
	has_redundancy?: number;
	degradation_priority?: number;
	offset?: number;
	subsamples?: unknown;
}

export interface ExtractionOptions {
	nbSamples: number;
}

export class DataStream {
	// WARNING, the default is little endian, which is not what MP4 uses.
	constructor(buffer?: ArrayBuffer, byteOffset?: number, endianness?: boolean);
	getPosition(): number;

	get byteLength(): number;
	get buffer(): ArrayBuffer;
	set buffer(v: ArrayBuffer);
	get byteOffset(): number;
	set byteOffset(v: number);
	get dataView(): DataView;
	set dataView(v: DataView);

	seek(pos: number): void;
	isEof(): boolean;

	mapFloat32Array(length: number, e?: boolean): Float32Array;
	mapFloat64Array(length: number, e?: boolean): Float64Array;
	mapInt16Array(length: number, e?: boolean): Int16Array;
	mapInt32Array(length: number, e?: boolean): Int32Array;
	mapInt8Array(length: number): Int8Array;
	mapUint16Array(length: number, e?: boolean): Uint16Array;
	mapUint32Array(length: number, e?: boolean): Uint32Array;
	mapUint8Array(length: number): Uint8Array;

	readInt32Array(length: number, endianness?: boolean): Int32Array;
	readInt16Array(length: number, endianness?: boolean): Int16Array;
	readInt8Array(length: number): Int8Array;
	readUint32Array(length: number, endianness?: boolean): Uint32Array;
	readUint16Array(length: number, endianness?: boolean): Uint16Array;
	readUint8Array(length: number): Uint8Array;
	readFloat64Array(length: number, endianness?: boolean): Float64Array;
	readFloat32Array(length: number, endianness?: boolean): Float32Array;

	readInt32(endianness?: boolean): number;
	readInt16(endianness?: boolean): number;
	readInt8(): number;
	readUint32(endianness?: boolean): number;
	//readUint32Array(length: unknown, e: unknown): unknown
	readUint24(): number;
	readUint16(endianness?: boolean): number;
	readUint8(): number;
	//readUint64(): unknown
	readFloat32(endianness?: boolean): number;
	readFloat64(endianness?: boolean): number;
	//readCString(length: number): unknown
	//readString(length: number, encoding: unknown): unknown

	static endianness: boolean;

	memcpy(
		dst: ArrayBufferLike,
		dstOffset: number,
		src: ArrayBufferLike,
		srcOffset: number,
		byteLength: number,
	): void;
	static memcpy(
		dst: unknown,
		dstOffset: number,
		src: unknown,
		srcOffset: number,
		byteLength: number,
	): void;

	// TODO I got bored porting all functions

	save(filename: string): void;
	shift(offset: number): void;

	writeInt32Array(arr: Int32Array, endianness?: boolean): void;
	writeInt16Array(arr: Int16Array, endianness?: boolean): void;
	writeInt8Array(arr: Int8Array): void;
	writeUint32Array(arr: Uint32Array, endianness?: boolean): void;
	writeUint16Array(arr: Uint16Array, endianness?: boolean): void;
	writeUint8Array(arr: Uint8Array): void;
	writeFloat64Array(arr: Float64Array, endianness?: boolean): void;
	writeFloat32Array(arr: Float32Array, endianness?: boolean): void;
	writeInt32(v: number, endianness?: boolean): void;
	writeInt16(v: number, endianness?: boolean): void;
	writeInt8(v: number): void;
	writeUint32(v: number, endianness?: boolean): void;
	writeUint16(v: number, endianness?: boolean): void;
	writeUint8(v: number): void;
	writeFloat32(v: number, endianness?: boolean): void;
	writeFloat64(v: number, endianness?: boolean): void;
	writeUCS2String(s: string, endianness?: boolean, length?: number): void;
	writeString(s: string, encoding?: string, length?: number): void;
	writeCString(s: string, length?: number): void;
	writeUint64(v: number): void;
	writeUint24(v: number): void;
	adjustUint32(pos: number, v: number): void;

	static LITTLE_ENDIAN: boolean;
	static BIG_ENDIAN: boolean;

	readCString(length: number): unknown;
	readInt64(): unknown;
	readString(length: number, encoding: string): unknown;
	readUint64(): unknown;
	writeStruct(structDefinition: unknown, struct: unknown): void;
	writeType(t: unknown, v: unknown, struct: unknown): unknown;

	static arrayToNative(array: unknown, arrayIsLittleEndian: boolean): unknown;
	static flipArrayEndianness(array: unknown): unknown;
	static nativeToEndian(array: unknown, littleEndian: boolean): unknown;
}

export interface TrackOptions {
	id?: number;
	type?: string;
	width?: number;
	height?: number;
	duration?: number;
	layer?: number;
	timescale?: number;
	media_duration?: number;
	language?: string;
	hdlr?: string;

	// video
	avcDecoderConfigRecord?: unknown;
	hevcDecoderConfigRecord?: unknown;

	// audio
	balance?: number;
	channel_count?: number;
	samplesize?: number;
	samplerate?: number;

	//captions
	namespace?: string;
	schema_location?: string;
	auxiliary_mime_types?: string;

	description?: BoxParser.Box;
	description_boxes?: BoxParser.Box[];

	default_sample_description_index_id?: number;
	default_sample_duration?: number;
	default_sample_size?: number;
	default_sample_flags?: number;
}

export interface FileOptions {
	brands?: string[];
	timescale?: number;
	rate?: number;
	duration?: number;
	width?: number;
}

export interface SampleOptions {
	sample_description_index?: number;
	duration?: number;
	cts?: number;
	dts?: number;
	is_sync?: boolean;
	is_leading?: number;
	depends_on?: number;
	is_depended_on?: number;
	has_redundancy?: number;
	degradation_priority?: number;
	subsamples?: unknown;
}

// TODO add the remaining functions
// TODO move to another module
export class ISOFile {
	constructor(stream?: DataStream);

	init(options?: FileOptions): ISOFile;
	addTrack(options?: TrackOptions): number;
	addSample(track: number, data: ArrayBuffer, options?: SampleOptions): Sample;

	createSingleSampleMoof(sample: Sample): BoxParser.moofBox;

	// helpers
	getTrackById(id: number): BoxParser.trakBox | undefined;
	getTrexById(id: number): BoxParser.trexBox | undefined;

	// boxes that are added to the root
	boxes: BoxParser.Box[];
	mdats: BoxParser.mdatBox[];
	moofs: BoxParser.moofBox[];

	ftyp?: BoxParser.ftypBox;
	moov?: BoxParser.moovBox;

	static writeInitializationSegment(
		ftyp: BoxParser.ftypBox,
		moov: BoxParser.moovBox,
		total_duration: number,
		sample_duration: number,
	): ArrayBuffer;

	// TODO add correct types; these are exported by dts-gen
	add(name: unknown): unknown;
	addBox(box: unknown): unknown;
	appendBuffer(ab: unknown, last: unknown): unknown;
	buildSampleLists(): void;
	buildTrakSampleLists(trak: unknown): void;
	checkBuffer(ab: unknown): unknown;
	createFragment(
		track_id: unknown,
		sampleNumber: unknown,
		stream_: unknown,
	): unknown;
	equal(b: unknown): unknown;
	flattenItemInfo(): void;
	flush(): void;
	getAllocatedSampleDataSize(): unknown;
	getBox(type: unknown): unknown;
	getBoxes(type: unknown, returnEarly: unknown): unknown;
	getBuffer(): unknown;
	getCodecs(): unknown;
	getInfo(): unknown;
	getItem(item_id: unknown): unknown;
	getMetaHandler(): unknown;
	getPrimaryItem(): unknown;
	getSample(trak: unknown, sampleNum: unknown): unknown;
	getTrackSample(track_id: unknown, number: unknown): unknown;
	getTrackSamplesInfo(track_id: unknown): unknown;
	hasIncompleteMdat(): unknown;
	hasItem(name: unknown): unknown;
	initializeSegmentation(): unknown;
	itemToFragmentedTrackFile(_options: unknown): unknown;
	parse(): void;
	print(output: unknown): void;
	processIncompleteBox(ret: unknown): unknown;
	processIncompleteMdat(): unknown;
	processItems(callback: unknown): void;
	processSamples(last: unknown): void;
	releaseItem(item_id: unknown): unknown;
	releaseSample(trak: unknown, sampleNum: unknown): unknown;
	releaseUsedSamples(id: unknown, sampleNum: unknown): void;
	resetTables(): void;
	restoreParsePosition(): unknown;
	save(name: unknown): void;
	saveParsePosition(): void;
	seek(time: unknown, useRap: unknown): unknown;
	seekTrack(time: unknown, useRap: unknown, trak: unknown): unknown;
	setExtractionOptions(id: unknown, user: unknown, options: unknown): void;
	setSegmentOptions(id: unknown, user: unknown, options: unknown): void;
	start(): void;
	stop(): void;
	unsetExtractionOptions(id: unknown): void;
	unsetSegmentOptions(id: unknown): void;
	updateSampleLists(): void;
	updateUsedBytes(box: unknown, ret: unknown): void;
	write(outstream: unknown): void;

	static initSampleGroups(
		trak: unknown,
		traf: unknown,
		sbgps: unknown,
		trak_sgpds: unknown,
		traf_sgpds: unknown,
	): void;
	static process_sdtp(sdtp: unknown, sample: unknown, number: unknown): void;
	static setSampleGroupProperties(
		trak: unknown,
		sample: unknown,
		sample_number: unknown,
		sample_groups_info: unknown,
	): void;
}

export namespace BoxParser {
	export class Box {
		size?: number;
		data?: Uint8Array;

		constructor(type?: string, size?: number);

		add(name: string): Box;
		addBox(box: Box): Box;
		set(name: string, value: unknown): void;
		addEntry(value: string, prop?: string): void;
		printHeader(output: unknown): void;
		write(stream: DataStream): void;
		writeHeader(stream: DataStream, msg?: string): void;
		computeSize(): void;

		// TODO add types for these
		parse(stream: unknown): void;
		parseDataAndRewind(stream: unknown): void;
		parseLanguage(stream: unknown): void;
		print(output: unknown): void;
	}

	// TODO finish add types for these classes
	export class AudioSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);

		getChannelCount(): unknown;
		getSampleRate(): unknown;
		getSampleSize(): unknown;
		isAudio(): unknown;
		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class CoLLBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class ContainerBox extends Box {
		constructor(type: unknown, size: unknown, uuid: unknown);

		parse(stream: unknown): void;
		print(output: unknown): void;
		write(stream: unknown): void;
	}

	export class FullBox extends Box {
		constructor(type: unknown, size: unknown, uuid: unknown);

		parse(stream: unknown): void;
		parseDataAndRewind(stream: unknown): void;
		parseFullHeader(stream: unknown): void;
		printHeader(output: unknown): void;
		writeHeader(stream: unknown): void;
	}

	export class HintSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);
	}

	export class MetadataSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);

		isMetadata(): unknown;
	}

	export class OpusSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class SampleEntry extends Box {
		constructor(
			type: unknown,
			size: unknown,
			hdr_size: unknown,
			start: unknown,
		);

		getChannelCount(): unknown;
		getCodec(): unknown;
		getHeight(): unknown;
		getSampleRate(): unknown;
		getSampleSize(): unknown;
		getWidth(): unknown;
		isAudio(): unknown;
		isHint(): unknown;
		isMetadata(): unknown;
		isSubtitle(): unknown;
		isVideo(): unknown;
		parse(stream: unknown): void;
		parseDataAndRewind(stream: unknown): void;
		parseFooter(stream: unknown): void;
		parseHeader(stream: unknown): void;
		write(stream: unknown): void;
		writeFooter(stream: unknown): void;
		writeHeader(stream: unknown): void;
	}

	export class SampleGroupEntry {
		constructor(type: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class SingleItemTypeReferenceBox extends ContainerBox {
		constructor(
			type: unknown,
			size: unknown,
			hdr_size: unknown,
			start: unknown,
		);

		parse(stream: unknown): void;
	}

	export class SingleItemTypeReferenceBoxLarge {
		constructor(
			type: unknown,
			size: unknown,
			hdr_size: unknown,
			start: unknown,
		);

		parse(stream: unknown): void;
	}

	export class SmDmBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class SubtitleSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);

		isSubtitle(): unknown;
	}

	export class SystemSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);
	}

	export class TextSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);
	}

	export class TrackGroupTypeBox extends FullBox {
		constructor(type: unknown, size: unknown);

		parse(stream: unknown): void;
	}

	export class TrackReferenceTypeBox extends ContainerBox {
		constructor(
			type: unknown,
			size: unknown,
			hdr_size: unknown,
			start: unknown,
		);

		parse(stream: unknown): void;

		write(stream: unknown): void;
	}

	export class VisualSampleEntry extends SampleEntry {
		constructor(type: unknown, size: unknown);

		getHeight(): unknown;
		getWidth(): unknown;
		isVideo(): unknown;
		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class a1lxBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class a1opBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class alstSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class auxCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class av01SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class av1CBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class avc1SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class avc2SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class avc3SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class avc4SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class avcCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class avllSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class avssSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class btrtBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class bxmlBox extends FullBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class clapBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class clefBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class clliBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class co64Box extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class colrBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class cprtBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class cslgBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class cttsBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		unpack(samples: unknown): void;
		write(stream: unknown): void;
	}

	export class dOpsBox extends ContainerBox {
		constructor(size?: number);

		parse(stream: DataStream): void;

		Version: number;
		OutputChannelCount: number;
		PreSkip: number;
		InputSampleRate: number;
		OutputGain: number;
		ChannelMappingFamily: number;

		// When channelMappingFamily != 0
		StreamCount?: number;
		CoupledCount?: number;
		ChannelMapping?: number[];
	}

	export class dac3Box extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class dec3Box extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class dfLaBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class dimmBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class dinfBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class dmaxBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class dmedBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class drefBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class drepBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class dtrtSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class edtsBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class elngBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class elstBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class emsgBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class encaSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class encmSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class encsSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class enctSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class encuSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class encvSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class enofBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class esdsBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class fielBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class freeBox extends Box {
		constructor(size: unknown);
	}

	export class frmaBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class ftypBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class hdlrBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class hev1SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class hinfBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class hmhdBox extends FullBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class hntiBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class hvc1SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class hvcCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class idatBox extends Box {
		constructor(size: unknown);
	}

	export class iinfBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class ilocBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class imirBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class infeBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class iodsBox extends FullBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class ipcoBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class ipmaBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class iproBox extends FullBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class iprpBox extends ContainerBox {
		constructor(size: unknown);
		ipmas: ipmaBox[];
	}

	export class irefBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class irotBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class ispeBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class kindBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;

		write(stream: unknown): void;
	}

	export class levaBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class lselBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class maxrBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class mdatBox extends Box {
		constructor(size: unknown);
	}

	export class mdcvBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class mdhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;

		write(stream: unknown): void;
	}

	export class mdiaBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class mecoBox extends Box {
		constructor(size: unknown);
	}

	export class mehdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;

		write(stream: unknown): void;
	}

	export class mereBox extends FullBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class metaBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class mettSampleEntry extends SampleEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class metxSampleEntry extends SampleEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class mfhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;

		write(stream: unknown): void;
	}

	export class mfraBox extends ContainerBox {
		constructor(size: unknown);
		tfras: tfraBox[];
	}

	export class mfroBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class minfBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class moofBox extends ContainerBox {
		constructor(size: unknown);
		trafs: trafBox[];
	}

	export class moovBox extends ContainerBox {
		constructor(size: unknown);
		traks: trakBox[];
		psshs: psshBox[];
	}

	export class mp4aSampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class msrcTrackGroupTypeBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class mvexBox extends ContainerBox {
		constructor(size: unknown);

		trexs: trexBox[];
	}

	export class mvhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		print(output: unknown): void;
		write(stream: unknown): void;
	}

	export class mvifSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class nmhdBox extends FullBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class npckBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class numpBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class padbBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class paspBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class paylBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class paytBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class pdinBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class pitmBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class pixiBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class pmaxBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class prftBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class profBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class prolSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class psshBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class rashSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class rinfBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class rollSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class saioBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class saizBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class sbgpBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;

		write(stream: unknown): void;
	}

	export class sbttSampleEntry extends SampleEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class schiBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class schmBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class scifSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class scnmSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class sdtpBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class seigSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class sencBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class sgpdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class sidxBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class sinfBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class skipBox extends Box {
		constructor(size: unknown);
	}

	export class smhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class ssixBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class stblBox extends ContainerBox {
		constructor(size: unknown);

		sgpds: sgpdBox[];
		sbgps: sbgpBox[];
	}

	export class stcoBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		unpack(samples: unknown): void;
		write(stream: unknown): void;
	}

	export class stdpBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class sthdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class stppSampleEntry extends SampleEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class strdBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class striBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class strkBox extends Box {
		constructor(size: unknown);
	}

	export class stsaSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class stscBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		unpack(samples: unknown): void;
		write(stream: unknown): void;
	}

	export class stsdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class stsgBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class stshBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class stssBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class stszBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		unpack(samples: unknown): void;
		write(stream: unknown): void;
	}

	export class sttsBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		unpack(samples: unknown): void;
		write(stream: unknown): void;
	}

	export class stviBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class stxtSampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
		parse(stream: unknown): void;
	}

	export class stypBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class stz2Box extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class subsBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class syncSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class taptBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class teleSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tencBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tfdtBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class tfhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class tfraBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tkhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		print(output: unknown): void;
		write(stream: unknown): void;
	}

	export class tmaxBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tminBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class totlBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tpayBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tpylBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class trafBox extends ContainerBox {
		constructor(size: unknown);
		truns: trunBox[];
		sgpd: sgpdBox[];
		sbgp: sbgpBox[];
	}

	export class trakBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class trefBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class trepBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class trexBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class trgrBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class trpyBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class trunBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class tsasSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tsclSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tselBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class tx3gSampleEntry extends SampleEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class txtCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class udtaBox extends ContainerBox {
		constructor(size: unknown);
		kinds: kindBox[];
	}

	export class viprSampleGroupEntry extends SampleGroupEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class vmhdBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
		write(stream: unknown): void;
	}

	export class vp08SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class vp09SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class vpcCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class vttCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class vttcBox extends ContainerBox {
		constructor(size: unknown);
	}

	export class vvc1SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class vvcCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class vvcNSampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class vvi1SampleEntry extends SampleEntry {
		constructor(size: unknown);

		getCodec(): unknown;
	}

	export class vvnCBox extends ContainerBox {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export class vvs1SampleEntry extends SampleEntry {
		constructor(size: unknown);
	}

	export class wvttSampleEntry extends SampleEntry {
		constructor(size: unknown);

		parse(stream: unknown): void;
	}

	export const BASIC_BOXES: string[];
	export const CONTAINER_BOXES: string[][];
	export const DIFF_BOXES_PROP_NAMES: string[];
	export const DIFF_PRIMITIVE_ARRAY_PROP_NAMES: string[];
	export const ERR_INVALID_DATA: number;
	export const ERR_NOT_ENOUGH_DATA: number;
	export const FULL_BOXES: string[];
	export const OK: number;
	export const SAMPLE_ENTRY_TYPE_AUDIO: string;
	export const SAMPLE_ENTRY_TYPE_HINT: string;
	export const SAMPLE_ENTRY_TYPE_METADATA: string;
	export const SAMPLE_ENTRY_TYPE_SUBTITLE: string;
	export const SAMPLE_ENTRY_TYPE_SYSTEM: string;
	export const SAMPLE_ENTRY_TYPE_TEXT: string;
	export const SAMPLE_ENTRY_TYPE_VISUAL: string;
	export const TFHD_FLAG_BASE_DATA_OFFSET: number;
	export const TFHD_FLAG_DEFAULT_BASE_IS_MOOF: number;
	export const TFHD_FLAG_DUR_EMPTY: number;
	export const TFHD_FLAG_SAMPLE_DESC: number;
	export const TFHD_FLAG_SAMPLE_DUR: number;
	export const TFHD_FLAG_SAMPLE_FLAGS: number;
	export const TFHD_FLAG_SAMPLE_SIZE: number;
	export const TKHD_FLAG_ENABLED: number;
	export const TKHD_FLAG_IN_MOVIE: number;
	export const TKHD_FLAG_IN_PREVIEW: number;
	export const TRUN_FLAGS_CTS_OFFSET: number;
	export const TRUN_FLAGS_DATA_OFFSET: number;
	export const TRUN_FLAGS_DURATION: number;
	export const TRUN_FLAGS_FIRST_FLAG: number;
	export const TRUN_FLAGS_FLAGS: number;
	export const TRUN_FLAGS_SIZE: number;
	export const UUIDs: string[];
	export const boxCodes: string[];
	export const containerBoxCodes: unknown[];
	export const fullBoxCodes: unknown[];

	export const sampleEntryCodes: {
		Audio: string[];
		Hint: unknown[];
		Metadata: string[];
		Subtitle: string[];
		System: string[];
		Text: string[];
		Visual: string[];
	};

	export const sampleGroupEntryCodes: unknown[];

	export const trackGroupTypes: unknown[];

	export function addSubBoxArrays(subBoxNames: unknown): void;
	export function boxEqual(box_a: unknown, box_b: unknown): unknown;
	export function boxEqualFields(box_a: unknown, box_b: unknown): unknown;
	export function createBoxCtor(type: unknown, parseMethod: unknown): void;
	export function createContainerBoxCtor(
		type: unknown,
		parseMethod: unknown,
		subBoxNames: unknown,
	): void;
	export function createEncryptedSampleEntryCtor(
		mediaType: unknown,
		type: unknown,
		parseMethod: unknown,
	): void;
	export function createFullBoxCtor(type: unknown, parseMethod: unknown): void;
	export function createMediaSampleEntryCtor(
		mediaType: unknown,
		parseMethod: unknown,
		subBoxNames: unknown,
	): void;
	export function createSampleEntryCtor(
		mediaType: unknown,
		type: unknown,
		parseMethod: unknown,
		subBoxNames: unknown,
	): void;
	export function createSampleGroupCtor(
		type: unknown,
		parseMethod: unknown,
	): void;
	export function createTrackGroupCtor(
		type: unknown,
		parseMethod: unknown,
	): void;
	export function createUUIDBox(
		uuid: unknown,
		isFullBox: unknown,
		isContainerBox: unknown,
		parseMethod: unknown,
	): void;
	export function decimalToHex(d: unknown, padding: unknown): unknown;
	export function initialize(): void;
	export function parseHex16(stream: unknown): unknown;
	export function parseOneBox(
		stream: unknown,
		headerOnly: unknown,
		parentSize: unknown,
	): unknown;
	export function parseUUID(stream: unknown): unknown;

	/* ???
	namespace UUIDBoxes {
		export class a2394f525a9b4f14a2446c427c648df4 {
			constructor(size: unknown)
		}

		export class a5d40b30e81411ddba2f0800200c9a66 {
			constructor(size: unknown)

			parse(stream: unknown): void
		}

		export class d08a4f1810f34a82b6c832d8aba183d3 {
			constructor(size: unknown)

			parse(stream: unknown): void
		}

		export class d4807ef2ca3946958e5426cb9e46a79f {
			constructor(size: unknown)

			parse(stream: unknown): void
		}
	}
	*/
}

// TODO Add types for the remaining classes found via dts-gen
export class MP4BoxStream {
	constructor(arrayBuffer: unknown);

	getEndPosition(): unknown;
	getLength(): unknown;
	getPosition(): unknown;
	isEos(): unknown;
	readAnyInt(size: unknown, signed: unknown): unknown;
	readCString(): unknown;
	readInt16(): unknown;
	readInt16Array(length: unknown): unknown;
	readInt32(): unknown;
	readInt32Array(length: unknown): unknown;
	readInt64(): unknown;
	readInt8(): unknown;
	readString(length: unknown): unknown;
	readUint16(): unknown;
	readUint16Array(length: unknown): unknown;
	readUint24(): unknown;
	readUint32(): unknown;
	readUint32Array(length: unknown): unknown;
	readUint64(): unknown;
	readUint8(): unknown;
	readUint8Array(length: unknown): unknown;
	seek(pos: unknown): unknown;
}

export class MultiBufferStream {
	constructor(buffer: unknown);

	addUsedBytes(nbBytes: unknown): void;
	cleanBuffers(): void;
	findEndContiguousBuf(inputindex: unknown): unknown;
	findPosition(
		fromStart: unknown,
		filePosition: unknown,
		markAsUsed: unknown,
	): unknown;
	getEndFilePositionAfter(pos: unknown): unknown;
	getEndPosition(): unknown;
	getLength(): unknown;
	getPosition(): unknown;
	initialized(): unknown;
	insertBuffer(ab: unknown): void;
	logBufferLevel(info: unknown): void;
	mergeNextBuffer(): unknown;
	reduceBuffer(buffer: unknown, offset: unknown, newLength: unknown): unknown;
	seek(filePosition: unknown, fromStart: unknown, markAsUsed: unknown): unknown;
	setAllUsedBytes(): void;
}

export class Textin4Parser {
	constructor();

	parseConfig(data: unknown): unknown;
	parseSample(sample: unknown): unknown;
}

export class XMLSubtitlein4Parser {
	constructor();

	parseSample(sample: unknown): unknown;
}

export function MPEG4DescriptorParser(): unknown;

export namespace BoxParser {}

export namespace Log {
	export const LOG_LEVEL_ERROR = 4;
	export const LOG_LEVEL_WARNING = 3;
	export const LOG_LEVEL_INFO = 2;
	export const LOG_LEVEL_DEBUG = 1;

	export function debug(module: unknown, msg: unknown): void;
	export function error(module: unknown, msg: unknown): void;
	export function getDurationString(
		duration: unknown,
		_timescale: unknown,
	): unknown;
	export function info(module: unknown, msg: unknown): void;
	export function log(module: unknown, msg: unknown): void;
	export function printRanges(ranges: unknown): unknown;
	export function setLogLevel(level: unknown): void;
	export function warn(module: unknown, msg: unknown): void;
}

declare var FLAGS: Flags;
declare global {
	interface Window {
		FLAGS: Flags;
		__CAP__: {
			cameraWsPort: number;
			cameraOnlyMode?: boolean;
		};
	}
}
