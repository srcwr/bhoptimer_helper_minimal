
#include "nanoflann.hpp"
using namespace nanoflann;

struct Point
{
	float pos[3];
	int idx;
};

struct PointCloud
{
	const Point* pts;
	size_t pts_size;

	inline size_t kdtree_get_point_count() const { return pts_size; }
	inline float kdtree_get_pt(const size_t idx, const size_t dim) const { return pts[idx].pos[dim]; }

	template <class BBOX>
	bool kdtree_get_bbox(BBOX& /* bb */) const { return false; }
};

typedef KDTreeSingleIndexAdaptor<
	// L2_Simple_Adaptor<float, PointCloud>,
	L2_Adaptor<float, PointCloud>,
	PointCloud,
	3 /*dimensions*/
	> my_kd_tree_t;

class KDTreeContainer
{
public:
	PointCloud cloud;
	my_kd_tree_t *index;
	int startidx;
};

extern "C" {
void* nanoflann_shim_create_container(const Point* pts, size_t pts_size)
{
	KDTreeContainer *container = new KDTreeContainer();
	container->cloud.pts = pts;
	container->cloud.pts_size = pts_size;
	container->index = new my_kd_tree_t(3, container->cloud, {100});
	return container;
}
void nanoflann_shim_delete_container(void* object)
{
	KDTreeContainer *container = (KDTreeContainer *)object;
	delete container;
}
int nanoflann_shim_get_nearest(void* object, float *query_pt)
{
	KDTreeContainer *container = (KDTreeContainer *)object;
	float out_dist_sqr;
	size_t ret_index = 0;
	container->index->knnSearch(query_pt, 1, &ret_index, &out_dist_sqr);
	return container->cloud.pts[ret_index].idx;
}
size_t nanoflann_shim_get_used_memory(void* object)
{
	KDTreeContainer *container = (KDTreeContainer *)object;
	return container->index->usedMemory(*container->index);
}
} // extern "C"
